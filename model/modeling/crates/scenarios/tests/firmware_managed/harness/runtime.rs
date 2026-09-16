//! Scenario assembly of DMA hardware and an independently executing firmware CPU.

mod bus;
mod error;
mod run;
mod transport;

#[cfg(test)]
mod tests;

pub use bus::HostBus;
pub use error::{BuildError, RunError};

use hardware_interface_runtime::{
    InterruptEmitter, InterruptRuntime, PendingInterrupt, SoftwareCpu, pending_interrupt,
};
use pdma_model_contract::{DeviceRegisters, FIRMWARE_STATUS_BASE};
use pdma_model_dma::{CommandLoop, Device};
use pdma_model_host::runtime::Irq;
use pdma_model_hw_components::{AccessWindow, FirmwareStatus, WindowConfigError};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Debug)]
struct Hardware<M> {
    dma: Device<M>,
    window: AccessWindow,
    firmware: FirmwareStatus,
}

#[derive(Debug)]
struct Shared<M> {
    hardware: Mutex<Hardware<M>>,
    changed: Notify,
    interrupt: InterruptEmitter,
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
    pub runtime: InterruptRuntime<PendingInterrupt<Irq>>,
}

/// One managed channel, driven only when its scoped run future is polled.
#[derive(Debug)]
pub struct Managed<M> {
    shared: Arc<Shared<M>>,
    pipeline: CommandLoop,
    firmware_cpu: Arc<SoftwareCpu>,
}

impl<M> Managed<M> {
    /// Assembles hardware and disjoint software views; starts no tasks or firmware.
    pub fn new(memory: M) -> Result<(Self, Host<M>), BuildError> {
        let dma = Device::new(memory, 1).map_err(BuildError::Dma)?;
        let pipeline = dma.command_loop();
        let extent = DeviceRegisters::extent(1).map_err(BuildError::Layout)?;
        let window = AccessWindow::new(extent).map_err(BuildError::Window)?;
        let firmware = FirmwareStatus::new(
            usize::try_from(FIRMWARE_STATUS_BASE)
                .map_err(|_| BuildError::Window(WindowConfigError::AddressUnrepresentable))?,
        );
        let (interrupt, source) = pending_interrupt(Irq);
        let shared = Arc::new(Shared {
            hardware: Mutex::new(Hardware {
                dma,
                window,
                firmware,
            }),
            changed: Notify::new(),
            interrupt,
        });
        let host = Host {
            bus: HostBus {
                shared: Arc::clone(&shared),
            },
            runtime: InterruptRuntime::new(Arc::new(SoftwareCpu::new()), source),
        };
        Ok((
            Self {
                shared,
                pipeline,
                firmware_cpu: Arc::new(SoftwareCpu::new()),
            },
            host,
        ))
    }
}
