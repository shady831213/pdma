#[doc(hidden)]
pub mod block;
mod publication;
#[doc(hidden)]
pub mod ring_state;
#[doc(hidden)]
pub mod state;
mod submission;

pub(crate) use block::PreparedExactQueue;
pub(crate) use ring_state::{RingState, SubmissionRingState, exact_cursor};
pub(crate) use state::ActiveQueue;

use crate::abi::ring::PdmaControlError;
use crate::abi::ring::QueueStatus;
use crate::dma::PdmaMemoryMappings;
use crate::queue::io::{NormalizedFault, NormalizedFetch, PublicationOutcome};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{AbiEvent, AbiEventKind, DmaAddress, QueueRingLayouts};
use pdma_model_contract::semantic::BufferId;
use pdma_model_contract::semantic::{
    CompletionRecord, QueueActivation, QueueDirectoryEntry, QueueId,
};

#[derive(Debug)]
pub(crate) struct ExactQueueIo {
    active: Option<ActiveQueue>,
    pending_fault: Option<AbiEvent>,
}

impl ExactQueueIo {
    #[cfg(test)]
    pub(crate) const fn inactive_for_test() -> Self {
        Self {
            active: None,
            pending_fault: None,
        }
    }

    pub(crate) const fn active(activation: QueueActivation, layouts: QueueRingLayouts) -> Self {
        Self {
            active: Some(ActiveQueue::new(activation, layouts)),
            pending_fault: None,
        }
    }

    pub(crate) const fn ring_state(&self) -> Result<&ActiveQueue, PdmaControlError> {
        match &self.active {
            Some(active) => Ok(active),
            None => Err(PdmaControlError::InvalidQueue),
        }
    }

    pub(crate) const fn ring_state_mut(&mut self) -> Result<&mut ActiveQueue, PdmaControlError> {
        match &mut self.active {
            Some(active) => Ok(active),
            None => Err(PdmaControlError::InvalidQueue),
        }
    }

    pub(crate) fn status(&self) -> Result<QueueStatus, PdmaControlError> {
        self.ring_state().and_then(|active| active.status())
    }

    pub(crate) const fn pending_result(&self) -> Option<crate::abi::ring::PendingDmaResult> {
        match self.pending_fault {
            Some(event) => Some(crate::abi::ring::PendingDmaResult::Event(event)),
            None => None,
        }
    }
    pub(crate) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let active = self
            .ring_state()
            .map_err(|_| ModelError::DmaSubmissionState)?;
        submission::fetch(memory, active, entry.queue_id()).map(|fetched| {
            fetched.map(|fetched| normalize(memory, entry.completion_ring_id(), fetched))
        })
    }

    pub(crate) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let active = self
            .ring_state()
            .map_err(|_| ModelError::DmaSubmissionState)?;
        let fetched = submission::fetch(memory, active, entry.queue_id())?;
        Ok(fetched.map(|fetched| match fetched {
            submission::ExactFetch::Command(command) => NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            },
            submission::ExactFetch::Fault(queue, event) => {
                self.pending_fault = Some(event);
                NormalizedFetch::Fault(NormalizedFault {
                    queue,
                    source: fault_source(memory, event),
                })
            }
        }))
    }

    pub(crate) fn consume(&mut self) -> ModelResult<()> {
        let active = self
            .ring_state_mut()
            .map_err(|_| ModelError::DmaSubmissionState)?;
        submission::consume(active).map_err(|_| ModelError::DmaSubmissionState)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.ring_state().is_ok_and(submission::has_pending)
    }

    pub(crate) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        let Ok(active) = self.ring_state_mut() else {
            return PublicationOutcome::Retry;
        };
        publication::completion(memory, active, record)
    }

    pub(crate) const fn stage_execution_fault(
        event: pdma_model_contract::semantic::EventRecord,
    ) -> ModelResult<ModelError> {
        Err(ModelError::from_memory_fault(
            event.buffer(),
            event.error_kind(),
        ))
    }

    pub(crate) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
    ) -> super::io::FaultPublication {
        let Some(event) = self.pending_fault else {
            return super::io::FaultPublication::Retry;
        };
        let outcome = self
            .ring_state_mut()
            .map_or(PublicationOutcome::Retry, |active| {
                publication::event(memory, active, event)
            });
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

    pub(crate) fn completion_blocked(&self, _publication_slots: usize) -> bool {
        self.ring_state()
            .map_or(true, |active| ring_is_full(active, true))
    }

    pub(crate) fn fault_blocked(&self, _compat_publication_slots: usize) -> bool {
        self.ring_state()
            .map_or(true, |active| ring_is_full(active, false))
    }

    pub(crate) fn records_pending(&self, completion: bool) -> ModelResult<bool> {
        let active = self
            .ring_state()
            .map_err(|_| ModelError::DmaSubmissionState)?;
        if completion {
            Ok(active.completion.producer() != active.completion.consumer())
        } else {
            Ok(active.event.producer() != active.event.consumer())
        }
    }
}

fn normalize(
    memory: &PdmaMemoryMappings,
    completion_ring: pdma_model_contract::semantic::CompletionRingId,
    fetched: submission::ExactFetch,
) -> NormalizedFetch {
    match fetched {
        submission::ExactFetch::Command(command) => NormalizedFetch::Command {
            command,
            completion_ring,
        },
        submission::ExactFetch::Fault(queue, event) => NormalizedFetch::Fault(NormalizedFault {
            queue,
            source: fault_source(memory, event),
        }),
    }
}

fn fault_source(memory: &PdmaMemoryMappings, event: AbiEvent) -> ModelError {
    let (kind, locator, detail0, detail1, _, _, _) = event.fault().tuple();
    match kind {
        AbiEventKind::UnmappedDmaAddress => ModelError::UnknownBuffer(BufferId::new(usize::MAX)),
        AbiEventKind::RangeOverflow => ModelError::RangeOverflow,
        AbiEventKind::OutOfBounds => memory
            .region_ending_at(DmaAddress::new(locator))
            .and_then(|region| {
                let len = usize::try_from(detail0).ok()?;
                let end = detail1.checked_sub(region.base().raw())?;
                usize::try_from(end)
                    .ok()
                    .map(|end| ModelError::OutOfBounds {
                        buffer: region.object(),
                        len,
                        end,
                    })
            })
            .unwrap_or(ModelError::DmaSubmissionState),
        AbiEventKind::InvalidVersion
        | AbiEventKind::InvalidSize
        | AbiEventKind::InvalidOpcode
        | AbiEventKind::InvalidFlags
        | AbiEventKind::InvalidReserved
        | AbiEventKind::InvalidIndirectCount
        | AbiEventKind::InvalidIndirectAlignment
        | AbiEventKind::ValueOutOfRange => ModelError::DmaSubmissionState,
    }
}

fn ring_is_full(active: &ActiveQueue, completion: bool) -> bool {
    if completion {
        active.completion.is_full()
    } else {
        active.event.is_full()
    }
}
