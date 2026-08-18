//! Async-native structured PDMA trace sink and JSON export.

mod event;
mod json;
mod sink;

use pdma_model_contract::semantic::desc::ExternalCommand;
use pdma_model_contract::semantic::ids::CommandId;
use pdma_model_contract::semantic::trace::TraceOperation;

pub use sink::TraceSink;

pub(crate) const fn operation_from_external(command: &ExternalCommand) -> TraceOperation {
    event::operation_from_external(command)
}

pub(crate) const fn command_id(command: &ExternalCommand) -> CommandId {
    event::command_id(command)
}
