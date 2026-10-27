#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod traits;
#[cfg(feature = "tokio")]
mod wire;

pub use traits::{InterruptEnvelope, InterruptHandler, InterruptSource};
#[cfg(feature = "tokio")]
pub use wire::{InterruptEmitter, InterruptReceiver, interrupt_wire};
