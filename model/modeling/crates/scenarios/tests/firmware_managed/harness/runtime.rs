//! Scenario assembly of DMA hardware and an independently executing firmware CPU.

mod build;
mod bus;
mod error;
mod interrupts;
mod memory;
/// Scenario-owned register placement and integration validation.
pub mod platform;
mod run;
mod transport;

#[cfg(test)]
mod tests;

pub use bus::HostBus;
pub use error::{AccessError, BuildError, PlatformConfigError, RunError};
pub use interrupts::{HostInterrupts, HostIrq, HostVector};

use hw_model_io::memory::MemoryAccess;
use hw_model_irq::{InterruptEmitter, InterruptReceiver, interrupt_wire};
use hw_model_swenv::{InterruptRuntime, SoftwareEnv};
use memory::RoutedMemory;
use pdma_model_contract::dma::DmaAccess;
use pdma_model_dma::Dma;
use pdma_model_host::runtime::Irq;
use pdma_model_hw_components::{
    context::ContextTable, control::ControlChannel, firmware::FirmwareStatus,
    windows::ChannelWindows,
};
use pdma_model_software::control::{ControlIrq, ManagementAddresses};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
struct Hardware<M> {
    dma: Dma<RoutedMemory<M>, DmaAccess>,
    windows: Arc<std::sync::Mutex<ChannelWindows>>,
    firmware: FirmwareStatus,
    control: Arc<std::sync::Mutex<ControlChannel>>,
    contexts: Option<Arc<std::sync::Mutex<ContextTable>>>,
}

#[derive(Debug)]
struct Shared<M> {
    addresses: ManagementAddresses,
    hardware: Mutex<Hardware<M>>,
    destinations: Vec<InterruptEmitter>,
    channel_count: usize,
    to_firmware: InterruptEmitter,
    to_host: InterruptEmitter,
    to_events: InterruptEmitter,
    timer: Arc<transport::SimulationTimer>,
}

#[derive(Debug)]
struct ManagementBus<M>(Arc<Shared<M>>);

#[derive(Debug)]
struct Driver<M>(Arc<Shared<M>>);

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
pub struct Managed<M: MemoryAccess>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    shared: Arc<Shared<M>>,
    hardware: Box<pdma_model_dma::Hardware>,
    firmware_runtime: InterruptRuntime<InterruptReceiver<ControlIrq>>,
    firmware_env: Arc<SoftwareEnv>,
    firmware_memory: memory::ControlMemory<M>,
    function_count: usize,
}
