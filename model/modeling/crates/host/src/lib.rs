//! Async PDMA host model.

pub mod adapter;
pub mod compat;
pub mod runtime;
pub mod semantic;

pub use semantic::{
    AsyncDriver, DriverError, DriverIrqRecord, DriverPort, DriverResult, ManualDriver, PortError,
    PortResult,
};
