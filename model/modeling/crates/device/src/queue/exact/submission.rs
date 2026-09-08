use super::ActiveQueue;
use super::PdmaControlError;
use crate::adapter::command::decode;
use crate::memory::PdmaMemoryMappings;
use crate::operation::DecodedCommand;
use crate::queue::io::{NormalizedFault, NormalizedFetch};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandView, AbiEvent, AbiEventKind, DescriptorDecodeError, DmaAddress,
};
use pdma_model_contract::semantic::{BufferId, CommandId, CompletionRingId, QueueId};

#[derive(Debug, Clone)]
pub(super) enum ExactFetch {
    Command(DecodedCommand),
    Fault(QueueId, AbiEvent),
}

pub(super) fn fetch(
    memory: &PdmaMemoryMappings,
    active: &ActiveQueue,
    queue_id: QueueId,
) -> ModelResult<Option<ExactFetch>> {
    if !active.submission.has_captured_entry() {
        return Ok(None);
    }
    let ring = active.submission.ring();
    let slot = ring
        .entry_address(ring.consumer())
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let bytes = memory
        .read_memory(slot, u64::from(ring.layout().entry_size()))
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let (exact_command_id, command_id) = command_id(&bytes)?;
    let exact_queue_id =
        u64::try_from(queue_id.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
    match AbiCommand::decode(&bytes) {
        Ok(command) => Ok(Some(ExactFetch::Command(decode(command, command_id)))),
        Err(DescriptorDecodeError::Violation(violation)) => Ok(Some(ExactFetch::Fault(
            queue_id,
            AbiEvent::new(
                exact_queue_id,
                exact_command_id,
                violation.into_abi_fault(slot),
            ),
        ))),
        Err(DescriptorDecodeError::Codec(_) | DescriptorDecodeError::MetadataMismatch) => {
            Err(ModelError::DmaSubmissionState)
        }
    }
}

pub(super) fn consume(active: &mut ActiveQueue) -> Result<(), PdmaControlError> {
    active.advance_submission_consumer()
}

pub(super) fn has_pending(active: &ActiveQueue) -> bool {
    active.submission.has_captured_entry()
}

pub(super) fn normalize(
    memory: &PdmaMemoryMappings,
    completion_ring: CompletionRingId,
    fetched: ExactFetch,
) -> NormalizedFetch {
    match fetched {
        ExactFetch::Command(command) => NormalizedFetch::Command {
            command,
            completion_ring,
        },
        ExactFetch::Fault(queue, event) => NormalizedFetch::Fault(NormalizedFault {
            queue,
            source: fault_source(memory, event),
        }),
    }
}

pub(super) fn fault_source(memory: &PdmaMemoryMappings, event: AbiEvent) -> ModelError {
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

fn command_id(bytes: &[u8]) -> ModelResult<(u64, CommandId)> {
    let view = AbiCommandView::new(bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let raw = view
        .header()
        .and_then(|header| header.command_id())
        .map_err(|_| ModelError::DmaSubmissionState)?;
    usize::try_from(raw)
        .map(|model| (raw, CommandId::new(model)))
        .map_err(|_| ModelError::DmaSubmissionState)
}
