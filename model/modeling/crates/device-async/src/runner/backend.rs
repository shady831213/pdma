use super::AsyncStallReason;
use super::native::NativeDevice;
use pdma_model_contract::ResourceConfig;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BackendStepStatus {
    Progressed,
    Idle,
    Stalled(AsyncStallReason),
}

#[derive(Debug)]
pub(super) enum RunnerBackend {
    Native(NativeDevice),
}

impl RunnerBackend {
    pub(super) fn native() -> Self {
        Self::native_with_resource_config(ResourceConfig::default())
    }

    pub(super) fn native_with_resource_config(config: ResourceConfig) -> Self {
        if config == ResourceConfig::default() {
            Self::Native(NativeDevice::new())
        } else {
            Self::Native(NativeDevice::with_resource_config(config))
        }
    }

    pub(super) fn step(&mut self) -> ModelResult<BackendStepStatus> {
        match self {
            Self::Native(device) => device.step(),
        }
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        match self {
            Self::Native(device) => device.create_queue(config),
        }
    }

    pub(super) fn allocate_buffer(&mut self, len: usize) -> BufferId {
        match self {
            Self::Native(device) => device.allocate_buffer(len),
        }
    }

    pub(super) fn write_buffer(
        &mut self,
        id: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        match self {
            Self::Native(device) => device.write_buffer(id, offset, data),
        }
    }

    pub(super) fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        match self {
            Self::Native(device) => device.read_buffer(id, offset, len),
        }
    }

    pub(super) fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        match self {
            Self::Native(device) => device.submit_command(queue_id, command),
        }
    }

    pub(super) fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        match self {
            Self::Native(device) => device.ring_doorbell(queue_id),
        }
    }

    pub(super) fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        match self {
            Self::Native(device) => device.poll_completion(completion_ring_id),
        }
    }

    pub(super) fn poll_queue_event(
        &mut self,
        queue_id: QueueId,
    ) -> ModelResult<Option<EventRecord>> {
        match self {
            Self::Native(device) => device.poll_queue_event(queue_id),
        }
    }
}
