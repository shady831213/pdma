//! Async PDMA host model.
#![cfg_attr(
    test,
    allow(
        clippy::items_after_test_module,
        reason = "the crate-private legacy regression harness is compiled only for host unit tests"
    )
)]

#[cfg(test)]
mod context;
mod error;
#[cfg(test)]
mod irq_delivery;

pub mod adapters;
pub mod compat;
pub mod interrupt;
pub mod register;
pub mod semantic;

pub use error::{ModelError, ModelResult};
pub use semantic::{AsyncDriver, DriverIrqRecord, DriverPort, ManualDriver};
