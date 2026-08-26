//! Canonical split memory/register PDMA host port.

mod bus;
mod capability;
mod error;
mod port;
mod types;

pub use crate::semantic::DriverPort;
pub use bus::BusQueueStatus;
pub use capability::{PdmaBatchAccess, PdmaExactInterruptClaim, PdmaMemoryProvisioning};
pub use error::{
    PdmaBatchError, PdmaClaimError, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegisterAccessError, PdmaRegisterPortError, RegisterError,
};
pub use port::PdmaRegisterPort;
pub use types::{
    PdmaClaimResult, PdmaDmaWrite, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest,
    PdmaSubmissionBatch,
};
