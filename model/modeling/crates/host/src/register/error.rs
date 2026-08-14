use hardware_interface_runtime::{MemoryAccessError, ObservationGateError};
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResultDetails};
use std::{error::Error, fmt};

/// Underlying host-owned provisioning cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaProvisioningCause {
    /// Fixed-width address arithmetic was exhausted.
    AddressExhausted,
    /// Backing storage reservation was exhausted.
    StorageExhausted,
    /// No committed mapping contains the access start.
    Unmapped,
    /// An access extends beyond its containing mapping.
    CrossesRegion,
    /// A proposal overlaps an existing mapping.
    OverlappingRegion,
    /// Mapping metadata is inconsistent.
    InvalidRegion,
    /// The allocator advanced after preparation.
    StaleProposal {
        /// Generation captured by the proposal.
        expected: u64,
        /// Current allocator generation.
        actual: u64,
    },
}

impl fmt::Display for PdmaProvisioningCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA provisioning cause: {self:?}")
    }
}
impl Error for PdmaProvisioningCause {}

/// Host-owned mapping allocation/preparation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaProvisioningError {
    /// The device-owned allocator rejected the operation.
    Provisioning(PdmaProvisioningCause),
    /// Runtime memory rejected a checked access.
    Access(MemoryAccessError),
    /// A host length cannot be represented by the device allocator.
    LengthOutOfRange,
    /// Logical and optional-storage metadata disagree.
    InvalidProjection,
}

impl fmt::Display for PdmaProvisioningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA provisioning failed: {self:?}")
    }
}
impl Error for PdmaProvisioningError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Provisioning(source) => Some(source),
            Self::Access(source) => Some(source),
            Self::LengthOutOfRange | Self::InvalidProjection => None,
        }
    }
}

/// Host projection of one raw register endpoint failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaRegisterAccessError {
    /// The exact generated register map rejected the access.
    Access(MmioAccessError),
    /// Ordered observation emission failed.
    Observation(ObservationGateError),
}
impl fmt::Display for PdmaRegisterAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA register access failed: {self:?}")
    }
}
impl Error for PdmaRegisterAccessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Access(source) => Some(source),
            Self::Observation(source) => Some(source),
        }
    }
}

/// Host projection of a closed batch failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaBatchError {
    /// Mapping preparation or commit failed.
    Provisioning(PdmaProvisioningError),
    /// A fixed register access was structurally invalid.
    Access(MmioAccessError),
    /// The terminal command returned a non-success result.
    Result(MmioResultDetails),
    /// The request did not satisfy the closed batch grammar.
    InvalidShape,
    /// Ordered observation emission failed.
    Observation(ObservationGateError),
}
impl fmt::Display for PdmaBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA batch failed: {self:?}")
    }
}
impl Error for PdmaBatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Provisioning(source) => Some(source),
            Self::Access(source) => Some(source),
            Self::Observation(source) => Some(source),
            Self::Result(_) | Self::InvalidShape => None,
        }
    }
}

/// Host projection of readiness/compound-claim failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaClaimError {
    /// The exact claim access was structurally invalid.
    Access(MmioAccessError),
    /// Ordered claim observation emission failed.
    Observation(ObservationGateError),
}
impl fmt::Display for PdmaClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA exact interrupt claim failed: {self:?}")
    }
}
impl Error for PdmaClaimError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Access(source) => Some(source),
            Self::Observation(source) => Some(source),
        }
    }
}

/// Transitional Task 21 register procedure error retained through Task 26.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterError {
    /// A register request was structurally rejected.
    Access(crate::core::PortError),
    /// A structurally valid request produced an invalid response.
    Response(crate::core::PortError),
}

impl From<crate::ModelError> for RegisterError {
    fn from(source: crate::ModelError) -> Self {
        Self::Response(source.into())
    }
}

impl fmt::Display for RegisterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(_) => formatter.write_str("host register access failed"),
            Self::Response(_) => formatter.write_str("host register response failed"),
        }
    }
}

impl Error for RegisterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Access(source) | Self::Response(source) => Some(source),
        }
    }
}
