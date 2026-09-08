mod block;
mod error;
mod publication;
mod ring_state;
mod state;
mod status;
mod submission;

#[cfg(test)]
mod pressure_tests;

pub use error::PdmaControlError;
pub(crate) use ring_state::RingState;
pub(crate) use state::ActiveQueue;
pub use status::PendingDmaResult;

use crate::memory::PdmaMemoryMappings;
use crate::queue::io::{NormalizedFault, NormalizedFetch, PublicationOutcome};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, QueueRingLayouts};
use pdma_model_contract::semantic::{CompletionRingId, QueueId};

#[derive(Debug)]
pub(super) struct ExactQueueIo {
    active: ActiveQueue,
    pending_fault: Option<AbiEvent>,
}

impl ExactQueueIo {
    pub(super) const fn active(layouts: QueueRingLayouts) -> Self {
        Self {
            active: ActiveQueue::new(layouts),
            pending_fault: None,
        }
    }

    pub(super) const fn ring_state(&self) -> &ActiveQueue {
        &self.active
    }

    pub(super) const fn ring_state_mut(&mut self) -> &mut ActiveQueue {
        &mut self.active
    }

    pub(super) const fn pending_result(&self) -> Option<PendingDmaResult> {
        match self.pending_fault {
            Some(event) => Some(PendingDmaResult::Event(event)),
            None => None,
        }
    }
    pub(super) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let active = self.ring_state();
        submission::fetch(memory, active, queue).map(|fetched| {
            fetched.map(|fetched| {
                submission::normalize(memory, CompletionRingId::new(queue.raw()), fetched)
            })
        })
    }

    pub(super) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let active = self.ring_state();
        let fetched = submission::fetch(memory, active, queue)?;
        Ok(fetched.map(|fetched| match fetched {
            submission::ExactFetch::Command(command) => NormalizedFetch::Command {
                command,
                completion_ring: CompletionRingId::new(queue.raw()),
            },
            submission::ExactFetch::Fault(queue, event) => {
                self.pending_fault = Some(event);
                NormalizedFetch::Fault(NormalizedFault {
                    queue,
                    source: submission::fault_source(memory, event),
                })
            }
        }))
    }

    pub(super) fn consume(&mut self) -> ModelResult<()> {
        let active = self.ring_state_mut();
        submission::consume(active).map_err(|_| ModelError::DmaSubmissionState)
    }

    pub(super) fn has_pending(&self) -> bool {
        submission::has_pending(self.ring_state())
    }

    pub(super) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        record: AbiCompletion,
    ) -> PublicationOutcome {
        let active = self.ring_state_mut();
        publication::completion(memory, active, record)
    }

    pub(super) const fn stage_execution_fault(&mut self, event: AbiEvent) {
        self.pending_fault = Some(event);
    }

    pub(super) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
    ) -> super::io::FaultPublication {
        let Some(event) = self.pending_fault else {
            return super::io::FaultPublication::Retry;
        };
        let outcome = publication::event(memory, self.ring_state_mut(), event);
        if outcome == PublicationOutcome::Published {
            let trace = if let (Ok(queue), Ok(command)) = (
                usize::try_from(event.origin_queue_id).map(QueueId::new),
                usize::try_from(event.command_id)
                    .map(pdma_model_contract::semantic::CommandId::new),
            ) {
                Some((queue, command))
            } else {
                None
            };
            self.pending_fault = None;
            return super::io::FaultPublication::Exact(trace);
        }
        super::io::FaultPublication::Retry
    }

    pub(super) fn completion_blocked(&self, publication_slots: usize) -> bool {
        publication_blocked(self.active.completion, publication_slots)
    }

    pub(super) fn fault_blocked(&self, publication_slots: usize) -> bool {
        publication_blocked(self.active.event, publication_slots)
    }

    pub(super) fn records_pending(&self, completion: bool) -> bool {
        let active = self.ring_state();
        if completion {
            active.completion.producer() != active.completion.consumer()
        } else {
            active.event.producer() != active.event.consumer()
        }
    }
}

fn publication_blocked<R: pdma_model_contract::abi::AbiRingEntry>(
    ring: RingState<R>,
    publication_slots: usize,
) -> bool {
    let occupied = ring.producer().raw() - ring.consumer().raw();
    ring.is_full() || usize::try_from(occupied).map_or(true, |used| used >= publication_slots)
}
