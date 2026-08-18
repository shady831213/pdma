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
mod driver;
mod error;
#[cfg(test)]
mod irq_delivery;

pub mod adapters;
pub mod compat;
pub mod core;
pub mod interrupt;
pub mod register;

pub use driver::{AsyncDriver, DriverIrqRecord, ManualDriver};
pub use error::{ModelError, ModelResult};
pub use register::DriverPort;
