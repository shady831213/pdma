#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "tokio")]
mod pending;
mod traits;

#[cfg(feature = "tokio")]
pub use pending::{InterruptEmitter, PendingInterrupt, pending_interrupt};
pub use traits::{InterruptEnvelope, InterruptHandler, InterruptSource};
