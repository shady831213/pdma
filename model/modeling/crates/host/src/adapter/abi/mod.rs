//! Host adapters that project semantic operations onto the exact ABI.

mod device_access;
mod interrupt;
mod memory;
mod register;

pub use device_access::AsyncDeviceAccess;
pub use interrupt::{PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSourceError};
pub use memory::PdmaHostMemory;
pub use register::{
    DriverPort, PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningCause,
    PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError, PdmaRegisterPort,
    PdmaRegisterPortError, RegisterQueueStatus,
};
