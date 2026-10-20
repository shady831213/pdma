//! Peripheral hardware behavior grouped by the object that owns it.
mod atomic_routing;
mod context;
mod control;
mod firmware;
mod platform_tags;
mod routing_errors;
/// Shared backing for hardware memory-routing tests.
pub mod support;
mod windows;
