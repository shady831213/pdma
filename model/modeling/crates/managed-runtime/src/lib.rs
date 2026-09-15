//! Concrete assembly of DMA hardware and an independently executing firmware CPU.

mod bus;
mod error;
mod interrupt;
mod run;
mod transport;

pub use bus::HostBus;
pub use error::{BuildError, RunError};
pub use interrupt::Notifications;

use hardware_interface_runtime::{InterruptRuntime, SoftwareCpu};
use pdma_model_contract::DeviceRegisters;
use pdma_model_dma::{CommandLoop, Device};
use pdma_model_hw_components::AccessWindow;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Debug)]
struct Hardware<M> {
    dma: Device<M>,
    window: AccessWindow,
}

#[derive(Debug)]
struct Shared<M> {
    hardware: Mutex<Hardware<M>>,
    changed: Notify,
    notifications: Notifications,
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
    pub runtime: InterruptRuntime<Notifications>,
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
        let notifications = Notifications::new();
        let shared = Arc::new(Shared {
            hardware: Mutex::new(Hardware { dma, window }),
            changed: Notify::new(),
            notifications: notifications.clone(),
        });
        let host = Host {
            bus: HostBus {
                shared: Arc::clone(&shared),
            },
            runtime: InterruptRuntime::new(Arc::new(SoftwareCpu::new()), notifications),
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
