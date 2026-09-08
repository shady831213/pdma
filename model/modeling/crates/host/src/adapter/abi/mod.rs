//! Host adapters that project semantic operations onto the exact ABI.

mod device_access;
mod interrupt;
mod register;

pub use device_access::AsyncDeviceAccess;
pub use interrupt::{
    PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSource, PdmaInterruptSourceError,
};
pub use register::{
    DriverPort, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim, PdmaIrqAccess,
    PdmaIrqCoordinator, PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningCause,
    PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError, PdmaRegisterPort,
    PdmaRegisterPortError, RegisterQueueStatus,
};
