mod submission;

use super::QueueFetch;
use crate::dma_space::DmaSpace;
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::{CompletionRingId, QueueId};

#[derive(Debug)]
pub(in crate::runner::native) struct DmaBusQueueIo;

impl DmaBusQueueIo {
    pub(super) const fn new() -> Self {
        Self
    }

    pub(super) fn fetch(
        dma_space: &DmaSpace,
        bus_control: &BusControlState,
        queue_id: QueueId,
        completion_ring: CompletionRingId,
    ) -> ModelResult<Option<QueueFetch>> {
        submission::fetch(dma_space, bus_control, queue_id, completion_ring)
    }

    pub(super) fn consume(bus_control: &mut BusControlState, queue_id: QueueId) -> ModelResult<()> {
        submission::consume(bus_control, queue_id).map_err(|_| ModelError::DmaSubmissionState)
    }

    pub(super) fn has_pending(bus_control: &BusControlState, queue_id: QueueId) -> bool {
        submission::has_pending(bus_control, queue_id)
    }
}
