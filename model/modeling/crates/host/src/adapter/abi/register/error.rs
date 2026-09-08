use hardware_interface_runtime::CpuContextError;
use hardware_interface_runtime::{MemoryAccessError, ObservationGateError};
use pdma_model_contract::abi::mmio::{MmioAccessError, ResponseCodecError};
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
    /// A region overlaps an existing mapping.
    OverlappingRegion,
    /// Mapping metadata is inconsistent.
    InvalidRegion,
}

impl fmt::Display for PdmaProvisioningCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA provisioning cause: {self:?}")
    }
}
impl Error for PdmaProvisioningCause {}

/// Host-owned mapping allocation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaProvisioningError {
    /// Runtime memory provisioning rejected the operation.
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

/// Host projection of readiness or sequential claim failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdmaClaimError {
    /// The exact claim access was structurally invalid.
    Access(MmioAccessError),
    /// Ordered claim observation emission failed.
    Observation(ObservationGateError),
    /// The result register rejected the command or contained an unknown code.
    CommandResult {
        #[doc = "Untruncated result-register value."]
        code: u64,
    },
    /// The response bank cannot represent a notification.
    MalformedResponse,
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
            Self::CommandResult { .. } | Self::MalformedResponse => None,
        }
    }
}

impl From<PdmaRegisterAccessError> for PdmaClaimError {
    fn from(source: PdmaRegisterAccessError) -> Self {
        match source {
            PdmaRegisterAccessError::Access(source) => Self::Access(source),
            PdmaRegisterAccessError::Observation(source) => Self::Observation(source),
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

impl From<PdmaRegisterPortError> for crate::semantic::PortError {
    fn from(source: PdmaRegisterPortError) -> Self {
        use crate::semantic::{PortBackendError, PortBackendKind};

        match source {
            PdmaRegisterPortError::Cpu(source) => Self::Cpu(source),
            PdmaRegisterPortError::Semantic(source) => source,
            PdmaRegisterPortError::Provisioning(source) => {
                Self::Backend(PortBackendError::new(PortBackendKind::Provisioning, source))
            }
            PdmaRegisterPortError::Register(source) => {
                Self::Backend(PortBackendError::new(PortBackendKind::Register, source))
            }
            PdmaRegisterPortError::Claim(source) => {
                Self::Backend(PortBackendError::new(PortBackendKind::Claim, source))
            }
            PdmaRegisterPortError::Memory(source) => {
                Self::Backend(PortBackendError::new(PortBackendKind::Memory, source))
            }
            PdmaRegisterPortError::Response(source) => {
                Self::Backend(PortBackendError::new(PortBackendKind::Response, source))
            }
        }
    }
}
