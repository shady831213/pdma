mod publication;
mod submission;

use super::QueueFetch;
use crate::dma_space::DmaSpace;
use crate::runner::bus::control::BusControlState;
use crate::{ModelError, ModelResult};
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

    pub(super) fn publish_completion(
        dma_space: &mut DmaSpace,
        bus_control: &mut BusControlState,
        queue_id: QueueId,
        record: pdma_model_contract::CompletionRecord,
    ) -> bool {
        matches!(
            publication::completion(dma_space, bus_control, queue_id, record),
            publication::PublicationOutcome::Published
        )
    }

    pub(super) fn publish_event(
        dma_space: &mut DmaSpace,
        bus_control: &mut BusControlState,
        event: pdma_model_contract::bridge::AbiEvent,
    ) -> bool {
        matches!(
            publication::event(dma_space, bus_control, event),
            publication::PublicationOutcome::Published
        )
    }
}
