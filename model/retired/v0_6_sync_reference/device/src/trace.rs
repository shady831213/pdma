//! Structured PDMA trace sink and Chrome/Perfetto JSON export.

mod event;
mod json;
mod sink;

use pdma_model_contract::trace::TraceOperation;

pub use sink::TraceSink;

pub(crate) const fn command_id(
    command: &pdma_model_contract::desc::ExternalCommand,
) -> pdma_model_contract::ids::CommandId {
    event::command_id(command)
}

pub(crate) const fn operation_from_external(
    command: &pdma_model_contract::desc::ExternalCommand,
) -> TraceOperation {
    event::operation_from_external(command)
}
