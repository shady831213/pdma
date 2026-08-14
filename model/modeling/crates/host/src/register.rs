//! Canonical split memory/register PDMA host port.

mod capability;
mod error;
mod port;
mod types;

pub use capability::{PdmaBatchAccess, PdmaExactInterruptClaim, PdmaMemoryProvisioning};
pub use error::{
    PdmaBatchError, PdmaClaimError, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegisterAccessError, RegisterError,
};
pub use port::PdmaRegisterPort;
pub use types::{
    PdmaClaimResult, PdmaDmaWrite, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest,
    PdmaSubmissionBatch,
};

pub use crate::bus::{
    AsyncDeviceBusStage, AsyncPdmaBus, BusDmaStage, BusDmaWrite, BusQueueStatus, BusSubmission,
    MmioBusControlError, MmioDeviceBus, MmioDeviceTransport, QueueControlWrite,
};
