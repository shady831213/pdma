//! Concrete and transitional host adapters.

mod async_device_access;

pub use crate::port::{BusControlErrorAdapter, BusDevicePort, BusDmaErrorAdapter};
pub use async_device_access::{AsyncDeviceAccess, AsyncDevicePreparedRegions};

crate::error::define_layer_error!(AdapterError, "host adapter operation failed");
