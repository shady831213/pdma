//! SDK/driver-like async submission and completion polling model.

mod accounting;
mod irq;
mod submission;

pub use irq::DriverIrqRecord;

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{EventRouting, QueueConfig, QueueConfigHandle, SoftwareOwnerId};

use crate::AsyncDevicePort;

/// Async software driver model owning queue setup and local ring-space accounting.
///
/// Cancelling submission before device acceptance restores its slot on the next accounting
/// access while keeping the command ID consumed. After acceptance, cancellation retains both the
/// queued command and its consumed slot, including cancellation during the doorbell operation.
#[derive(Debug, Clone)]
pub struct AsyncDriver {
    queue_config: QueueConfigHandle,
    accounting: accounting::SharedAccounting,
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
        let queue_config = accounting.poll_valid(device.create_queue(config)).await?;
        Ok(Self {
            queue_config,
            accounting,
        })
    }

    /// Sets up one command queue for an intentional manual-observability path.
    pub async fn setup_manual_on(
        device: &mut impl AsyncDevicePort,
        command_depth: usize,
        completion_depth: usize,
    ) -> ModelResult<Self> {
        Self::setup_manual_with_config_on(
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
    pub async fn setup_manual_with_config_on(
        device: &mut impl AsyncDevicePort,
        config: QueueConfig,
    ) -> ModelResult<Self> {
        let accounting = accounting::SharedAccounting::manual(config.command_depth());
        let queue_config = accounting.poll_valid(device.create_queue(config)).await?;
        Ok(Self {
            queue_config,
            accounting,
        })
    }

    /// Allocates a device-visible buffer through an async device port.
    pub async fn allocate_buffer_on(
        &self,
        device: &mut impl AsyncDevicePort,
        len: usize,
    ) -> BufferId {
        device.allocate_buffer(len).await
    }

    /// Writes a complete buffer payload at offset zero through an async device port.
    pub async fn write_buffer_on(
        &self,
        device: &mut impl AsyncDevicePort,
        buffer: BufferId,
        data: &[u8],
    ) -> ModelResult<()> {
        device.write_buffer(buffer, 0, data).await
    }

    /// Reads a checked buffer payload at offset zero through an async device port.
    pub async fn read_buffer_on(
        &self,
        device: &impl AsyncDevicePort,
        buffer: BufferId,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        device.read_buffer(buffer, 0, len).await
    }

    /// Polls one completion through an async device port and reclaims one local command slot.
    pub async fn poll_completion_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
    ) -> ModelResult<Option<CompletionRecord>> {
        let completion = self
            .accounting
            .poll_valid(device.poll_completion(self.queue_config.completion_ring_id()))
            .await?;
        if completion.is_some() {
            self.accounting.reclaim()?;
        }
        Ok(completion)
    }

    /// Polls one event through an async device port and reclaims one local command slot.
    pub async fn poll_event_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
    ) -> ModelResult<Option<EventRecord>> {
        let event = match self.queue_config.event_routing() {
            EventRouting::QueueLocal => {
                self.accounting
                    .poll_valid(device.poll_queue_event(self.queue_config.queue_id()))
                    .await?
            }
        };
        if event.is_some() {
            self.accounting.reclaim()?;
        }
        Ok(event)
    }

    /// Queue ID owned by this driver instance.
    pub const fn queue_id(&self) -> QueueId {
        self.queue_config.queue_id()
    }

    /// Completion ring ID owned by this driver instance.
    pub const fn completion_ring_id(&self) -> CompletionRingId {
        self.queue_config.completion_ring_id()
    }

    /// Queue configuration handle owned by this driver instance.
    pub const fn queue_config_handle(&self) -> QueueConfigHandle {
        self.queue_config
    }
}
