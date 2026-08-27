//! Async PDMA host model.

#[cfg(test)]
mod context;
#[cfg(test)]
mod irq_delivery;

pub mod adapters;
pub mod compat;
pub mod interrupt;
pub mod register;
pub mod semantic;

pub use semantic::{
    AsyncDriver, DriverError, DriverIrqRecord, DriverPort, DriverResult, ManualDriver, PortError,
    PortResult,
};
