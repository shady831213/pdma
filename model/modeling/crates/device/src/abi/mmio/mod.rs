//! Exact numeric MMIO command dispatch and register storage.

pub mod dispatch;
pub(crate) mod global;
#[doc(hidden)]
pub mod irq;
pub mod queue;
pub mod reset;
