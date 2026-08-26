//! Exact ABI bus, MMIO, and register-endpoint adapters.

pub(in crate::runner) mod bus;
pub(in crate::runner) mod mmio;
pub(in crate::runner) mod register_endpoint;

pub use register_endpoint::{MmioRegisterEndpoint, MmioRegisterError};
