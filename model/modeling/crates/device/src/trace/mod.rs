//! Async-native structured PDMA trace sink and JSON export.

mod event;
mod json;
#[doc(hidden)]
pub(crate) mod sink;

pub(crate) use sink::TraceSink;
