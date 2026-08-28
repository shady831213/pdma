mod publication;
mod submission;

use super::{FrontendBehavior, NormalizedFault, NormalizedFetch, PublicationOutcome};
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::abi::bus::control::BusControlState;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{AbiEvent, AbiEventKind, DmaAddress};
use pdma_model_contract::semantic::BufferId;
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{CompletionRecord, QueueDirectoryEntry, QueueId};

#[derive(Debug)]
pub(in crate::runner::execution) struct ExactFrontend {
    pending_fault: Option<AbiEvent>,
}

impl ExactFrontend {
    pub(super) const fn new() -> Self {
        Self {
            pending_fault: None,
        }
    }

    pub(super) const fn pending_fault(&self) -> Option<AbiEvent> {
        self.pending_fault
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
        .map(|fetched| fetched.map(|fetched| normalize(memory, fetched)))
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
            submission::ExactFetch::Fault(queue, event) => {
                self.pending_fault = Some(event);
                NormalizedFetch::Fault(NormalizedFault {
                    queue,
                    source: fault_source(memory, event),
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

fn normalize(memory: &PdmaMemoryMappings, fetched: submission::ExactFetch) -> NormalizedFetch {
    match fetched {
        submission::ExactFetch::Command(command) => NormalizedFetch::Command(command),
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
