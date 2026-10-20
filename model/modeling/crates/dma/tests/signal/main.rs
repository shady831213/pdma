//! Signal accesses and waiting commands through the DMA hardware bus.
mod atomic;
mod cancellation;
mod failures;
/// Memory backend with pre-access fault injection for hardware tests.
pub mod fault_memory;
mod lifecycle;
mod operations;
/// Shared signal hardware assembly and bus helpers.
pub mod support;
