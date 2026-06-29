//! Structured PDMA trace sink and Chrome/Perfetto JSON export.

mod event;
mod json;
mod sink;

pub use event::{TraceEvent, TraceEventKind, TraceOperation};
pub use sink::TraceSink;

pub(crate) const fn command_id(command: &crate::desc::ExternalCommand) -> crate::ids::CommandId {
    event::command_id(command)
}
