//! Compatibility surfaces retained until the final migration task.

mod async_device;
mod semantic_device;

pub use crate::error::{ModelError, ModelResult};
pub use async_device::AsyncDevicePort;
pub use semantic_device::SemanticDevicePort;

crate::error::define_layer_error!(CompatError, "host compatibility operation failed");
