//! Actor-style synchronous driver methods over a device port.

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, CopyCommand, CopyRequest, ExternalCommand,
    StridedCopyCommand, StridedCopyRequest, VectorAddCommand, VectorAddRequest,
};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CommandId};
use pdma_model_contract::queue::{EventRouting, QueueConfig, SoftwareOwnerId};

use crate::DevicePort;

use super::Driver;

impl Driver {
    /// Sets up one command queue through a synchronous device port.
    pub fn setup_on(
        device: &mut impl DevicePort,
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
    }

    /// Sets up one command queue from an explicit queue configuration through a device port.
    pub fn setup_with_config_on(
        device: &mut impl DevicePort,
        config: QueueConfig,
    ) -> ModelResult<Self> {
        let queue_config = device.create_queue(config)?;
        Ok(Self {
            queue_config,
            next_command: 0,
            free_command_slots: config.command_depth(),
        })
    }

    /// Allocates a device-visible buffer through a device port.
    pub fn allocate_buffer_on(&self, device: &mut impl DevicePort, len: usize) -> BufferId {
        device.allocate_buffer(len)
    }

    /// Writes a complete buffer payload at offset zero through a device port.
    pub fn write_buffer_on(
        &self,
        device: &mut impl DevicePort,
        buffer: BufferId,
        data: &[u8],
    ) -> ModelResult<()> {
        device.write_buffer(buffer, 0, data)
    }

    /// Reads a checked buffer payload at offset zero through a device port.
    pub fn read_buffer_on(
        &self,
        device: &impl DevicePort,
        buffer: BufferId,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        device.read_buffer(buffer, 0, len)
    }

    /// Emits a copy descriptor through a device port and rings the doorbell.
    pub fn submit_copy_on(
        &mut self,
        device: &mut impl DevicePort,
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
    }

    /// Emits a strided copy descriptor through a device port and rings the doorbell.
    pub fn submit_strided_copy_on(
        &mut self,
        device: &mut impl DevicePort,
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
    }

    /// Emits a broadcast copy descriptor through a device port and rings the doorbell.
    pub fn submit_broadcast_copy_on(
        &mut self,
        device: &mut impl DevicePort,
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
    }

    /// Emits a vector-add descriptor through a device port and rings the doorbell.
    pub fn submit_vector_add_on(
        &mut self,
        device: &mut impl DevicePort,
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
    }

    /// Polls one completion through a device port and reclaims one local command slot.
    pub fn poll_completion_on(
        &mut self,
        device: &mut impl DevicePort,
    ) -> ModelResult<Option<CompletionRecord>> {
        let completion = device.poll_completion(self.queue_config.completion_ring_id())?;
        if completion.is_some() {
            self.free_command_slots = self.free_command_slots.saturating_add(1);
        }
        Ok(completion)
    }

    /// Polls one event through a device port and reclaims one local command slot.
    pub fn poll_event_on(
        &mut self,
        device: &mut impl DevicePort,
    ) -> ModelResult<Option<EventRecord>> {
        let event = match self.queue_config.event_routing() {
            EventRouting::QueueLocal => device.poll_queue_event(self.queue_config.queue_id())?,
        };
        if event.is_some() {
            self.free_command_slots = self.free_command_slots.saturating_add(1);
        }
        Ok(event)
    }

    const fn reserve_command_slot(&self) -> ModelResult<()> {
        if self.free_command_slots == 0 {
            return Err(ModelError::DriverQueueFull);
        }
        Ok(())
    }

    fn submit_external_command_on(
        &mut self,
        device: &mut impl DevicePort,
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
