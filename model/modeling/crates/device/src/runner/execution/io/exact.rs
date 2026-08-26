mod publication;
mod submission;

use super::{FrontendBehavior, NormalizedFault, NormalizedFetch, PublicationOutcome};
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::abi::bus::control::BusControlState;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{CompletionRecord, QueueDirectoryEntry, QueueId};

#[derive(Debug)]
pub(in crate::runner::execution) struct ExactFrontend {
    pending_fault: Option<pdma_model_contract::binding::AbiEvent>,
}

impl ExactFrontend {
    pub(super) const fn new() -> Self {
        Self {
            pending_fault: None,
        }
    }

    pub(super) fn pending_fault_bytes(&self) -> Option<[u8; 64]> {
        self.pending_fault
            .and_then(|event| pdma_model_contract::translation::encode_event(event).ok())
    }
}

impl FrontendBehavior for ExactFrontend {
    fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        submission::fetch(
            memory,
            control,
            entry.queue_id(),
            entry.completion_ring_id(),
        )
        .map(|fetched| fetched.map(normalize))
    }

    fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let fetched = submission::fetch(
            memory,
            control,
            entry.queue_id(),
            entry.completion_ring_id(),
        )?;
        Ok(fetched.map(|fetched| match fetched {
            submission::ExactFetch::Command(command) => NormalizedFetch::Command(command),
            submission::ExactFetch::Fault(event) => {
                self.pending_fault = Some(event);
                NormalizedFetch::Fault(NormalizedFault {
                    queue: event.queue_id(),
                })
            }
        }))
    }

    fn consume(&mut self, control: &mut BusControlState, queue: QueueId) -> ModelResult<()> {
        submission::consume(control, queue).map_err(|_| ModelError::DmaSubmissionState)
    }

    fn has_pending(&self, control: &BusControlState, queue: QueueId) -> bool {
        submission::has_pending(control, queue)
    }

    fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        publication::completion(memory, control, queue, record)
    }

    fn stage_execution_fault(
        &mut self,
        fault: super::super::retirement::FaultRetirement,
    ) -> ModelResult<ModelError> {
        Err(ModelError::from_memory_fault(
            fault.fault.buffer(),
            fault.fault.error_kind(),
        ))
    }

    fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        trace: &mut crate::trace::TraceSink,
    ) -> PublicationOutcome {
        let Some(event) = self.pending_fault else {
            return PublicationOutcome::Retry;
        };
        let outcome = publication::event(memory, control, event);
        if outcome == PublicationOutcome::Published {
            trace.dma_event_published(event);
            self.pending_fault = None;
        }
        outcome
    }

    fn completion_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        _publication_slots: usize,
    ) -> bool {
        ring_is_full(control, queue, true)
    }

    fn fault_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        _compat_publication_slots: usize,
    ) -> bool {
        ring_is_full(control, queue, false)
    }

    fn records_pending(
        &self,
        control: &BusControlState,
        queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        let active = control
            .queue(EpochQueue::new(control.epoch(), queue))
            .map_err(|_| ModelError::DmaSubmissionState)?;
        if completion {
            Ok(active.completion.producer() != active.completion.consumer())
        } else {
            Ok(active.event.producer() != active.event.consumer())
        }
    }
}

fn normalize(fetched: submission::ExactFetch) -> NormalizedFetch {
    match fetched {
        submission::ExactFetch::Command(command) => NormalizedFetch::Command(command),
        submission::ExactFetch::Fault(event) => NormalizedFetch::Fault(NormalizedFault {
            queue: event.queue_id(),
        }),
    }
}

pub(in crate::runner::execution) fn decode_event(
    bytes: &[u8],
) -> Option<pdma_model_contract::binding::AbiEvent> {
    pdma_model_contract::translation::decode_event(bytes).ok()
}

pub(super) fn ring_is_full(control: &BusControlState, queue: QueueId, completion: bool) -> bool {
    control
        .queue(EpochQueue::new(control.epoch(), queue))
        .map_or(true, |active| {
            if completion {
                active.completion.is_full()
            } else {
                active.event.is_full()
            }
        })
}
