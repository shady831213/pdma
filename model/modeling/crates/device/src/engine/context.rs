use super::{DeviceExecution, io};
use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueDirectoryEntry, QueueId};

#[derive(Debug)]
pub(super) struct QueueContext {
    pub(super) entry: QueueDirectoryEntry,
    pub(super) frontend: io::QueueFrontend,
}

pub(super) struct NativeExecution<'a> {
    pub(super) device: &'a mut DeviceExecution,
    pub(super) dma_space: &'a mut PdmaMemoryMappings,
    pub(super) queue_control: &'a mut QueueControlState,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CommandRoute {
    pub(super) queue: QueueId,
    pub(super) command: CommandId,
    pub(super) completion_ring: CompletionRingId,
}
