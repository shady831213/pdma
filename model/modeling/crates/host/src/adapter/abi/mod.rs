//! Host adapters that project semantic operations onto the exact ABI.

mod device_access;
mod interrupt;
mod register;

pub use device_access::{AsyncDeviceAccess, AsyncDevicePreparedRegions};
pub use interrupt::{
    PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSource, PdmaInterruptSourceError,
};
pub use register::{
    DriverPort, PdmaBatchAccess, PdmaBatchError, PdmaClaimError, PdmaClaimResult, PdmaDmaWrite,
    PdmaExactInterruptClaim, PdmaMappedRegion, PdmaMemoryProvisioning, PdmaPreparedProposal,
    PdmaProvisioningCause, PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError,
    PdmaRegisterPort, PdmaRegisterPortError, PdmaSubmissionBatch, RegisterQueueStatus,
};
