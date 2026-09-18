//! Scenario assembly of DMA hardware and an independently executing firmware CPU.

mod build;
mod bus;
mod error;
mod interrupts;
/// Scenario-owned register placement and integration validation.
pub mod platform;
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
use pdma_model_dma::{CommandLoop, Dma};
use pdma_model_host::runtime::Irq;
use pdma_model_hw_components::{ChannelWindows, FirmwareStatus, FunctionControls};
use pdma_model_software::control::{ControlIrq, ManagementAddresses};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Debug)]
struct Hardware<M> {
    dma: Dma<M>,
    window: ChannelWindows,
    firmware: FirmwareStatus,
    control: FunctionControls,
}

#[derive(Debug)]
struct Shared<M> {
    addresses: ManagementAddresses,
    hardware: Mutex<Hardware<M>>,
    changed: Notify,
    destinations: Vec<Destination>,
    channel_count: usize,
    to_firmware: InterruptEmitter,
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

/// An instance-configured managed DMA, driven only while its run future is polled.
#[derive(Debug)]
pub struct Managed<M> {
    shared: Arc<Shared<M>>,
    pipeline: CommandLoop,
    firmware_runtime: InterruptRuntime<PendingInterrupt<ControlIrq>>,
    firmware_cpu: Arc<SoftwareCpu>,
    firmware_memory: M,
    function_count: usize,
}

#[derive(Debug)]
struct Destination {
    interrupt: InterruptEmitter,
    to_host: InterruptEmitter,
}
