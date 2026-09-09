use super::ActiveQueue;
use super::PdmaControlError;
use crate::adapter::command::decode;
use crate::operation::DecodedCommand;
use crate::queue::io::{NormalizedFault, NormalizedFetch};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandView, AbiEvent, AbiEventAccess, AbiEventStage, AbiFaultSite,
    DescriptorDecodeError,
};
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId};

#[derive(Debug, Clone)]
pub(super) enum ExactFetch {
    Command(DecodedCommand),
    Fault(QueueId, AbiEvent),
}

pub(super) fn fetch(
    memory: &SharedMemoryAccess,
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
    let length = u64::from(ring.layout().entry_size());
    let bytes = crate::memory::read(memory, slot, length).map_err(|error| {
        ModelError::MemoryFault(crate::memory::fault(
            error,
            length,
            AbiFaultSite::new(AbiEventStage::Descriptor, AbiEventAccess::Read, 0),
        ))
    })?;
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

pub(super) const fn normalize(
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
            source: ModelError::MemoryFault(event.fault()),
        }),
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
