//! Compatibility surfaces retained until the final migration task.

pub use crate::context::{CpuLocal, SoftwareCpu};
pub use crate::error::{ModelError, ModelResult};
pub use crate::port::AsyncDevicePort;

crate::error::define_layer_error!(CompatError, "host compatibility operation failed");
