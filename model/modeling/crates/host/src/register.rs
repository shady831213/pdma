//! Canonical split memory/register PDMA host port.

#[path = "register/../bus.rs"]
mod bus;
#[path = "register/capability.rs"]
mod capability;
#[path = "register/error.rs"]
mod error;
#[path = "register/../port.rs"]
mod port;
#[path = "register/port.rs"]
mod register_port;
#[path = "register/types.rs"]
mod types;

pub use bus::BusQueueStatus;
pub use capability::{PdmaBatchAccess, PdmaExactInterruptClaim, PdmaMemoryProvisioning};
pub use error::{
    PdmaBatchError, PdmaClaimError, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegisterAccessError, PdmaRegisterPortError, RegisterError,
};
pub use port::DriverPort;
pub use register_port::PdmaRegisterPort;
pub use types::{
    PdmaClaimResult, PdmaDmaWrite, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest,
    PdmaSubmissionBatch,
};
