//! SDK/driver-like async submission and completion polling model.

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, CopyCommand, CopyRequest, ExternalCommand,
    StridedCopyCommand, StridedCopyRequest, VectorAddCommand, VectorAddRequest,
};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::ids::{BufferId, CommandId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{EventRouting, QueueConfig, QueueConfigHandle, SoftwareOwnerId};

use crate::AsyncDevicePort;

/// Async software driver model owning queue setup and local ring-space accounting.
#[derive(Debug, Clone)]
pub struct AsyncDriver {
    queue_config: QueueConfigHandle,
    next_command: usize,
    free_command_slots: usize,
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
        let queue_config = device.create_queue(config).await?;
        Ok(Self {
            queue_config,
            next_command: 0,
            free_command_slots: config.command_depth(),
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

    /// Emits a copy descriptor through an async device port and rings the doorbell.
    pub async fn submit_copy_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
        request: CopyRequest,
    ) -> ModelResult<CommandId> {
        self.reserve_command_slot()?;
        let command_id = CommandId::new(self.next_command);
        let command = CopyCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, command_id, ExternalCommand::Copy(command))
            .await
    }

    /// Emits a strided copy descriptor through an async device port and rings the doorbell.
    pub async fn submit_strided_copy_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
        request: StridedCopyRequest,
    ) -> ModelResult<CommandId> {
        self.reserve_command_slot()?;
        let command_id = CommandId::new(self.next_command);
        let command = StridedCopyCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, command_id, ExternalCommand::StridedCopy(command))
            .await
    }

    /// Emits a broadcast copy descriptor through an async device port and rings the doorbell.
    pub async fn submit_broadcast_copy_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
        request: BroadcastCopyRequest,
    ) -> ModelResult<CommandId> {
        self.reserve_command_slot()?;
        let command_id = CommandId::new(self.next_command);
        let command = BroadcastCopyCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, command_id, ExternalCommand::BroadcastCopy(command))
            .await
    }

    /// Emits a vector-add descriptor through an async device port and rings the doorbell.
    pub async fn submit_vector_add_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
        request: VectorAddRequest,
    ) -> ModelResult<CommandId> {
        self.reserve_command_slot()?;
        let command_id = CommandId::new(self.next_command);
        let command = VectorAddCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command_on(device, command_id, ExternalCommand::VectorAdd(command))
            .await
    }

    /// Polls one completion through an async device port and reclaims one local command slot.
    pub async fn poll_completion_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
    ) -> ModelResult<Option<CompletionRecord>> {
        let completion = device
            .poll_completion(self.queue_config.completion_ring_id())
            .await?;
        if completion.is_some() {
            self.free_command_slots = self.free_command_slots.saturating_add(1);
        }
        Ok(completion)
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

    const fn reserve_command_slot(&self) -> ModelResult<()> {
        if self.free_command_slots == 0 {
            return Err(ModelError::DriverQueueFull);
        }
        Ok(())
    }

    async fn submit_external_command_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
        command_id: CommandId,
        command: ExternalCommand,
    ) -> ModelResult<CommandId> {
        self.next_command = self.next_command.saturating_add(1);
        device
            .submit_command(self.queue_config.queue_id(), command)
            .await?;
        self.free_command_slots -= 1;
        device.ring_doorbell(self.queue_config.queue_id()).await?;
        Ok(command_id)
    }
}
