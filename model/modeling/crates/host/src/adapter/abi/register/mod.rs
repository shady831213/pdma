//! Canonical split memory/register PDMA host port.

mod capability;
mod error;
mod port;
mod status;
mod types;

pub use crate::semantic::DriverPort;
pub use capability::{PdmaBatchAccess, PdmaExactInterruptClaim, PdmaMemoryProvisioning};
pub use error::{
    PdmaBatchError, PdmaClaimError, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegisterAccessError, PdmaRegisterPortError,
};
pub use port::PdmaRegisterPort;
pub use status::RegisterQueueStatus;
pub use types::{
    PdmaClaimResult, PdmaDmaWrite, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest,
    PdmaSubmissionBatch,
};
