//! Device-local interrupt routing, masking, latching, and notification state.

mod controller;
pub(crate) mod mmio;
pub(crate) mod source;

pub(crate) use controller::PdmaIrqController;
