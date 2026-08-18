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
    PdmaRegisterAccessError, RegisterError,
};
pub use port::DriverPort;
pub use register_port::PdmaRegisterPort;
pub use types::{
    PdmaClaimResult, PdmaDmaWrite, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest,
    PdmaSubmissionBatch,
};

#[cfg(test)]
pub(in crate::register) mod legacy_test_support {
    #![allow(
        clippy::redundant_pub_crate,
        reason = "crate-private re-exports are visible only to the legacy unit-test harness"
    )]

    pub(super) use super::bus::{
        AsyncPdmaBus, BusDmaStage, BusDmaWrite, BusQueueStatus, BusSubmission, MmioBusControlError,
        MmioDeviceBus, MmioDeviceTransport, QueueControlWrite,
    };
    pub(super) use super::port::{BusControlErrorAdapter, BusDevicePort, BusDmaErrorAdapter};
    pub(super) use crate::irq_delivery::{
        IrqDelivery, IrqHandler, IrqHandlerFuture, StatefulIrqHandler, irq_handler,
        stateful_irq_handler,
    };
}

#[cfg(test)]
#[path = "register/../legacy_integration_tests.rs"]
mod legacy_integration_tests;
