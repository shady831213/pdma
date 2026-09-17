//! Scenario assembly of DMA hardware and an independently executing firmware CPU.

mod bus;
mod error;
mod interrupts;
mod run;
mod transport;

#[cfg(test)]
mod tests;

pub use bus::HostBus;
pub use error::{BuildError, RunError};
pub use interrupts::{HostInterrupts, HostIrq, HostVector};

use hardware_interface_runtime::{
    InterruptEmitter, InterruptRuntime, PendingInterrupt, SoftwareCpu, pending_interrupt,
};
use pdma_model_contract::{CONTROL_CHANNEL_BASE, DeviceRegisters, FIRMWARE_STATUS_BASE};
use pdma_model_dma::{CommandLoop, Device};
use pdma_model_host::runtime::Irq;
use pdma_model_hw_components::{AccessWindow, ControlChannel, FirmwareStatus, WindowConfigError};
use pdma_model_software::control::ControlIrq;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Debug)]
struct Hardware<M> {
    dma: Device<M>,
    window: AccessWindow,
    firmware: FirmwareStatus,
    control: ControlChannel,
}

#[derive(Debug)]
struct Shared<M> {
    hardware: Mutex<Hardware<M>>,
    changed: Notify,
    interrupt: InterruptEmitter,
    to_firmware: InterruptEmitter,
    to_host: InterruptEmitter,
}

#[derive(Debug)]
struct ManagementBus<M>(Arc<Shared<M>>);

#[derive(Debug)]
struct Driver<M>(Arc<Shared<M>>);

/// Hardware access failure returned through the restricted Host bus.
pub type AccessError = pdma_model_hw_components::AccessError<pdma_model_dma::Error>;

/// Host-visible capabilities, without management access or DMA internals.
#[derive(Debug)]
pub struct Host<M> {
    /// Restricted work bus and read-only readiness view.
    pub bus: HostBus<M>,
    /// The Host CPU's independent interrupt execution environment.
    pub runtime: InterruptRuntime<HostInterrupts>,
}

/// One managed channel, driven only when its scoped run future is polled.
#[derive(Debug)]
pub struct Managed<M> {
    shared: Arc<Shared<M>>,
    pipeline: CommandLoop,
    firmware_runtime: InterruptRuntime<PendingInterrupt<ControlIrq>>,
    firmware_cpu: Arc<SoftwareCpu>,
    firmware_memory: M,
}

impl<M: Clone> Managed<M> {
    /// Assembles hardware and disjoint software views; starts no tasks or firmware.
    pub fn new(memory: M) -> Result<(Self, Host<M>), BuildError> {
        let dma = Device::new(memory.clone(), 1).map_err(BuildError::Dma)?;
        let pipeline = dma.command_loop();
        let extent = DeviceRegisters::extent(1).map_err(BuildError::Layout)?;
        let window = AccessWindow::new(extent).map_err(BuildError::Window)?;
        let firmware = FirmwareStatus::new(
            usize::try_from(FIRMWARE_STATUS_BASE)
                .map_err(|_| BuildError::Window(WindowConfigError::AddressUnrepresentable))?,
        );
        let (interrupt, dma_source) = pending_interrupt(HostIrq::Dma(Irq));
        let control = ControlChannel::new(
            usize::try_from(CONTROL_CHANNEL_BASE)
                .map_err(|_| BuildError::Window(WindowConfigError::AddressUnrepresentable))?,
        );
        let (to_firmware, firmware_source) = pending_interrupt(ControlIrq);
        let (to_host, control_source) = pending_interrupt(HostIrq::Control(ControlIrq));
        let shared = Arc::new(Shared {
            hardware: Mutex::new(Hardware {
                dma,
                window,
                firmware,
                control,
            }),
            changed: Notify::new(),
            interrupt,
            to_firmware,
            to_host,
        });
        let host = Host {
            bus: HostBus {
                shared: Arc::clone(&shared),
            },
            runtime: InterruptRuntime::new(
                Arc::new(SoftwareCpu::new()),
                HostInterrupts {
                    dma: dma_source,
                    control: control_source,
                    next: HostVector::Dma,
                },
            ),
        };
        let firmware_cpu = Arc::new(SoftwareCpu::new());
        Ok((
            Self {
                shared,
                pipeline,
                firmware_runtime: InterruptRuntime::new(Arc::clone(&firmware_cpu), firmware_source),
                firmware_cpu,
                firmware_memory: memory,
            },
            host,
        ))
    }
}
