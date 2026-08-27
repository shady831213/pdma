use hardware_interface_runtime::CpuContextError;
use hardware_interface_runtime::{MemoryAccessError, ObservationGateError};
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResultDetails, ResponseCodecError};
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

/// Typed failure returned by the canonical split-capability register port.
#[derive(Debug)]
pub enum PdmaRegisterPortError {
    /// The port was polled outside its owning software CPU.
    Cpu(CpuContextError),
    /// Host semantic validation rejected the request.
    Semantic(crate::semantic::PortError),
    /// Host-owned memory provisioning failed.
    Provisioning(PdmaProvisioningError),
    /// One generated register access failed.
    Register(PdmaRegisterAccessError),
    /// A closed queue batch failed.
    Batch(PdmaBatchError),
    /// Interrupt readiness or claim failed.
    Claim(PdmaClaimError),
    /// Checked DMA memory access failed.
    Memory(MemoryAccessError),
    /// A generated response view rejected bytes or a value.
    Response(ResponseCodecError),
}

impl fmt::Display for PdmaRegisterPortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical PDMA register port failed: {self:?}")
    }
}

impl Error for PdmaRegisterPortError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(source) => Some(source),
            Self::Semantic(source) => Some(source),
            Self::Provisioning(source) => Some(source),
            Self::Register(source) => Some(source),
            Self::Batch(source) => Some(source),
            Self::Claim(source) => Some(source),
            Self::Memory(source) => Some(source),
            Self::Response(source) => Some(source),
        }
    }
}

impl From<CpuContextError> for PdmaRegisterPortError {
    fn from(source: CpuContextError) -> Self {
        Self::Cpu(source)
    }
}
impl From<crate::semantic::PortError> for PdmaRegisterPortError {
    fn from(source: crate::semantic::PortError) -> Self {
        Self::Semantic(source)
    }
}
impl From<PdmaProvisioningError> for PdmaRegisterPortError {
    fn from(source: PdmaProvisioningError) -> Self {
        Self::Provisioning(source)
    }
}
impl From<PdmaRegisterAccessError> for PdmaRegisterPortError {
    fn from(source: PdmaRegisterAccessError) -> Self {
        Self::Register(source)
    }
}
impl From<PdmaBatchError> for PdmaRegisterPortError {
    fn from(source: PdmaBatchError) -> Self {
        Self::Batch(source)
    }
}
impl From<PdmaClaimError> for PdmaRegisterPortError {
    fn from(source: PdmaClaimError) -> Self {
        Self::Claim(source)
    }
}
impl From<MemoryAccessError> for PdmaRegisterPortError {
    fn from(source: MemoryAccessError) -> Self {
        Self::Memory(source)
    }
}
impl From<ResponseCodecError> for PdmaRegisterPortError {
    fn from(source: ResponseCodecError) -> Self {
        Self::Response(source)
    }
}
