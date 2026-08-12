//! SDK/driver-like async submission and completion polling model.

use crate::ModelResult;

mod accounting;
mod core;
mod irq;
mod submission;

pub use irq::DriverIrqRecord;

use core::DriverCore;
use pdma_model_contract::{
    BufferId, CompletionRecord, CompletionRingId, EpochIrqSource, EventRecord, EventRouting,
    QueueConfig, QueueConfigHandle, QueueId, SoftwareOwnerId,
};

use crate::AsyncDevicePort;

/// Async software driver model owning queue setup and local ring-space accounting.
///
/// Cancelling submission before device acceptance restores its slot on the next accounting
/// access while keeping the command ID consumed. After acceptance, cancellation retains both the
/// queued command and its consumed slot, including cancellation during the doorbell operation.
#[derive(Debug, Clone)]
pub struct AsyncDriver {
    core: DriverCore,
}

/// Explicit CPU-unbound driver facade for manual device observability.
///
/// Clones share command IDs, queue credit, and deferred reservation rollback state.
#[derive(Debug, Clone)]
pub struct ManualDriver {
    core: DriverCore,
}

impl AsyncDriver {
    /// Sets up one command queue through an async device port.
    pub async fn setup_on(
        device: &mut impl AsyncDevicePort,
        command_depth: usize,
        completion_depth: usize,
    ) -> ModelResult<Self> {
        Self::setup_with_config_on(
            device,
            QueueConfig::new(
                command_depth,
                completion_depth,
                EventRouting::QueueLocal,
                SoftwareOwnerId::new(0),
            ),
        )
        .await
    }

    /// Sets up one command queue from an explicit queue configuration through an async port.
    pub async fn setup_with_config_on(
        device: &mut impl AsyncDevicePort,
        config: QueueConfig,
    ) -> ModelResult<Self> {
        let accounting = accounting::SharedAccounting::capture_active(config.command_depth())?;
        let core = DriverCore::setup_on(device, config, accounting).await?;
        Ok(Self { core })
    }
}

impl ManualDriver {
    /// Sets up one command queue for an intentional manual-observability path.
    pub async fn setup_on(
        device: &mut impl AsyncDevicePort,
        command_depth: usize,
        completion_depth: usize,
    ) -> ModelResult<Self> {
        Self::setup_with_config_on(
            device,
            QueueConfig::new(
                command_depth,
                completion_depth,
                EventRouting::QueueLocal,
                SoftwareOwnerId::new(0),
            ),
        )
        .await
    }

    /// Sets up one manually driven queue from an explicit queue configuration.
    pub async fn setup_with_config_on(
        device: &mut impl AsyncDevicePort,
        config: QueueConfig,
    ) -> ModelResult<Self> {
        let accounting = accounting::SharedAccounting::manual(config.command_depth());
        let core = DriverCore::setup_on(device, config, accounting).await?;
        Ok(Self { core })
    }
}

macro_rules! impl_driver_data_plane {
    ($driver:ty) => {
        impl $driver {
            /// Allocates a device-visible buffer through an async device port.
            pub async fn allocate_buffer_on(
                &self,
                device: &mut impl AsyncDevicePort,
                len: usize,
            ) -> ModelResult<BufferId> {
                self.core.allocate_buffer_on(device, len).await
            }

            /// Writes a complete buffer payload at offset zero through an async device port.
            pub async fn write_buffer_on(
                &self,
                device: &mut impl AsyncDevicePort,
                buffer: BufferId,
                data: &[u8],
            ) -> ModelResult<()> {
                self.core.write_buffer_on(device, buffer, data).await
            }

            /// Reads a checked buffer payload at offset zero through an async device port.
            pub async fn read_buffer_on(
                &self,
                device: &impl AsyncDevicePort,
                buffer: BufferId,
                len: usize,
            ) -> ModelResult<Vec<u8>> {
                self.core.read_buffer_on(device, buffer, len).await
            }

            /// Polls one completion and reclaims one local command slot.
            pub async fn poll_completion_on(
                &mut self,
                device: &mut impl AsyncDevicePort,
            ) -> ModelResult<Option<CompletionRecord>> {
                self.core.poll_completion_on(device).await
            }

            /// Polls one event and reclaims one local command slot.
            pub async fn poll_event_on(
                &mut self,
                device: &mut impl AsyncDevicePort,
            ) -> ModelResult<Option<EventRecord>> {
                self.core.poll_event_on(device).await
            }

            /// Queue ID owned by this driver instance.
            pub const fn queue_id(&self) -> QueueId {
                self.core.queue_id()
            }

            /// Completion ring ID owned by this driver instance.
            pub const fn completion_ring_id(&self) -> CompletionRingId {
                self.core.completion_ring_id()
            }

            /// Queue configuration handle owned by this driver instance.
            pub const fn queue_config_handle(&self) -> QueueConfigHandle {
                self.core.queue_config_handle()
            }

            /// Epoch-qualified completion interrupt source owned by this driver.
            pub const fn completion_irq_source(&self) -> EpochIrqSource {
                self.core.queue_config_handle().completion_irq_source()
            }

            /// Epoch-qualified event interrupt source owned by this driver.
            pub const fn event_irq_source(&self) -> EpochIrqSource {
                self.core.queue_config_handle().event_irq_source()
            }
        }
    };
}

impl_driver_data_plane!(AsyncDriver);
impl_driver_data_plane!(ManualDriver);
