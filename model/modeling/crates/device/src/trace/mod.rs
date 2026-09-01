//! Async-native structured PDMA trace sink and JSON export.

mod event;
mod json;
#[doc(hidden)]
pub mod sink;

pub(crate) use sink::TraceSink;
