use pdma_model_contract::semantic::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::semantic::desc::ExternalCommand;
use pdma_model_contract::semantic::event::{EventErrorKind, EventRecord};
use pdma_model_contract::semantic::ids::CommandId;
use pdma_model_contract::semantic::trace::TraceOperation;

pub(super) const fn operation_from_external(command: &ExternalCommand) -> TraceOperation {
    match command {
        ExternalCommand::Copy(_) => TraceOperation::Copy,
        ExternalCommand::StridedCopy(_) => TraceOperation::StridedCopy,
        ExternalCommand::BroadcastCopy(_) => TraceOperation::BroadcastCopy,
        ExternalCommand::VectorAdd(_) => TraceOperation::VectorAdd,
    }
}

pub(super) const fn command_id(command: &ExternalCommand) -> CommandId {
    match command {
        ExternalCommand::Copy(copy) => copy.command_id(),
        ExternalCommand::StridedCopy(strided) => strided.command_id(),
        ExternalCommand::BroadcastCopy(broadcast) => broadcast.command_id(),
        ExternalCommand::VectorAdd(vector_add) => vector_add.command_id(),
    }
}

pub(super) const fn completion_detail(record: CompletionRecord) -> &'static str {
    match record.status() {
        CompletionStatus::Success => "status Success",
    }
}

pub(super) const fn event_detail(record: EventRecord) -> &'static str {
    match record.error_kind() {
        EventErrorKind::UnknownBuffer => "Error UnknownBuffer",
        EventErrorKind::RangeOverflow => "Error RangeOverflow",
        EventErrorKind::OutOfBounds { len: _, end: _ } => "Error OutOfBounds",
    }
}
