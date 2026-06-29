//! SDK/driver-like submission and completion polling model.

use crate::completion::CompletionRecord;
use crate::desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, CopyCommand, CopyRequest, ExternalCommand,
    StridedCopyCommand, StridedCopyRequest, VectorAddCommand, VectorAddRequest,
};
use crate::device::PdmaDevice;
use crate::event::EventRecord;
use crate::ids::{BufferId, CommandId, CompletionRingId, QueueId};
use crate::queue::{EventRouting, QueueConfig, QueueConfigHandle, SoftwareOwnerId};
use crate::{ModelError, ModelResult};

/// Software driver model owning queue setup and local ring-space accounting.
#[derive(Debug, Clone)]
pub struct Driver {
    queue_config: QueueConfigHandle,
    next_command: usize,
    free_command_slots: usize,
}

impl Driver {
    /// Sets up one command queue and bound completion ring on the device.
    pub fn setup(
        device: &mut PdmaDevice,
        command_depth: usize,
        completion_depth: usize,
    ) -> ModelResult<Self> {
        Self::setup_with_config(
            device,
            QueueConfig::new(
                command_depth,
                completion_depth,
                EventRouting::QueueLocal,
                SoftwareOwnerId::new(0),
            ),
        )
    }

    /// Sets up one command queue from an explicit queue configuration.
    pub fn setup_with_config(device: &mut PdmaDevice, config: QueueConfig) -> ModelResult<Self> {
        let queue_config = device.create_queue(config)?;
        Ok(Self {
            queue_config,
            next_command: 0,
            free_command_slots: config.command_depth(),
        })
    }

    /// Allocates a device-visible buffer.
    pub fn allocate_buffer(&self, device: &mut PdmaDevice, len: usize) -> BufferId {
        device.allocate_buffer(len)
    }

    /// Writes a complete buffer payload at offset zero.
    pub fn write_buffer(
        &self,
        device: &mut PdmaDevice,
        buffer: BufferId,
        data: &[u8],
    ) -> ModelResult<()> {
        device.write_buffer(buffer, 0, data)
    }

    /// Reads a checked buffer payload at offset zero.
    pub fn read_buffer(
        &self,
        device: &PdmaDevice,
        buffer: BufferId,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        device.read_buffer(buffer, 0, len)
    }

    /// Emits a copy descriptor to the command ring and rings the doorbell.
    pub fn submit_copy(
        &mut self,
        device: &mut PdmaDevice,
        request: CopyRequest,
    ) -> ModelResult<CommandId> {
        if self.free_command_slots == 0 {
            return Err(ModelError::DriverQueueFull);
        }
        let command_id = CommandId::new(self.next_command);
        let command = CopyCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command(device, command_id, ExternalCommand::Copy(command))
    }

    /// Emits a strided copy descriptor to the command ring and rings the doorbell.
    pub fn submit_strided_copy(
        &mut self,
        device: &mut PdmaDevice,
        request: StridedCopyRequest,
    ) -> ModelResult<CommandId> {
        if self.free_command_slots == 0 {
            return Err(ModelError::DriverQueueFull);
        }
        let command_id = CommandId::new(self.next_command);
        let command = StridedCopyCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command(device, command_id, ExternalCommand::StridedCopy(command))
    }

    /// Emits a broadcast copy descriptor to the command ring and rings the doorbell.
    pub fn submit_broadcast_copy(
        &mut self,
        device: &mut PdmaDevice,
        request: BroadcastCopyRequest,
    ) -> ModelResult<CommandId> {
        if self.free_command_slots == 0 {
            return Err(ModelError::DriverQueueFull);
        }
        let command_id = CommandId::new(self.next_command);
        let command = BroadcastCopyCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command(device, command_id, ExternalCommand::BroadcastCopy(command))
    }

    /// Emits a vector-add descriptor to the command ring and rings the doorbell.
    pub fn submit_vector_add(
        &mut self,
        device: &mut PdmaDevice,
        request: VectorAddRequest,
    ) -> ModelResult<CommandId> {
        if self.free_command_slots == 0 {
            return Err(ModelError::DriverQueueFull);
        }
        let command_id = CommandId::new(self.next_command);
        let command = VectorAddCommand::new(
            command_id,
            self.queue_config.queue_id(),
            self.queue_config.completion_ring_id(),
            request,
        );
        self.submit_external_command(device, command_id, ExternalCommand::VectorAdd(command))
    }

    /// Polls one completion and reclaims one local command-ring slot if present.
    pub fn poll_completion(
        &mut self,
        device: &mut PdmaDevice,
    ) -> ModelResult<Option<CompletionRecord>> {
        let completion = device.poll_completion(self.queue_config.completion_ring_id())?;
        if completion.is_some() {
            self.free_command_slots = self.free_command_slots.saturating_add(1);
        }
        Ok(completion)
    }

    /// Polls one event and reclaims one local command-ring slot if it belongs to this queue.
    pub fn poll_event(&mut self, device: &mut PdmaDevice) -> ModelResult<Option<EventRecord>> {
        let event = match self.queue_config.event_routing() {
            EventRouting::QueueLocal => device.poll_queue_event(self.queue_config.queue_id())?,
        };
        if event.is_some() {
            self.free_command_slots = self.free_command_slots.saturating_add(1);
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

    fn submit_external_command(
        &mut self,
        device: &mut PdmaDevice,
        command_id: CommandId,
        command: ExternalCommand,
    ) -> ModelResult<CommandId> {
        self.next_command = self.next_command.saturating_add(1);
        device.submit_command(self.queue_config.queue_id(), command)?;
        self.free_command_slots -= 1;
        device.ring_doorbell(self.queue_config.queue_id())?;
        Ok(command_id)
    }
}

#[cfg(test)]
mod tests {
    use super::Driver;
    use crate::ModelError;
    use crate::desc::CopyRequest;
    use crate::device::PdmaDevice;
    use crate::ids::CommandId;

    #[test]
    fn submit_copy_writes_command_and_uses_local_shadow_slots() {
        // Given: a driver with one command slot.
        let mut device = PdmaDevice::new();
        let mut driver = Driver::setup(&mut device, 1, 1).expect("queue setup is valid");
        let src = driver.allocate_buffer(&mut device, 4);
        let dst = driver.allocate_buffer(&mut device, 4);

        // When: one copy is submitted.
        let command_id = driver
            .submit_copy(&mut device, CopyRequest::new(src, dst, 4))
            .expect("first command has a free slot");

        // Then: the command ID is assigned and a second submit is locally rejected.
        assert_eq!(command_id, CommandId::new(0));
        assert_eq!(
            driver.submit_copy(&mut device, CopyRequest::new(src, dst, 4)),
            Err(ModelError::DriverQueueFull)
        );
    }
}
