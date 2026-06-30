//! Host-side PDMA software driver model.
//!
//! This crate owns the SDK-like setup, submission, polling, and local command
//! slot accounting surface used to drive the conceptual device model.

pub mod driver;
pub mod port;

pub use driver::Driver;
pub use port::DevicePort;
