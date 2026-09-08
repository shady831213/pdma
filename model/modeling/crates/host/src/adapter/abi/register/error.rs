use hardware_interface_runtime::CpuContextError;
use hardware_interface_runtime::{MemoryAccessError, ObservationGateError};
use pdma_model_contract::abi::registers::MmioAccessError;
use pdma_model_contract::semantic::DeviceEpoch;
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
#[derive(Debug, PartialEq, Eq)]
pub enum PdmaRegisterAccessError {
    /// The exact generated register map rejected the access.
    Access(MmioAccessError),
    /// Ordered observation emission failed.
    Observation(ObservationGateError),
    /// Hardware accepted the write before its observer failed.
    CommittedObservation(ObservationGateError),
    /// Hardware control rejected the write before changing state.
    Control(crate::PortError),
    /// The endpoint belongs to an earlier reset epoch.
    StaleEpoch {
        /// Current device epoch.
        expected: DeviceEpoch,
        /// Endpoint's bound epoch.
        actual: DeviceEpoch,
    },
}
impl PdmaRegisterAccessError {
    /// Reports whether the rejected observation follows a committed hardware write.
    pub const fn is_committed(&self) -> bool {
        matches!(self, Self::CommittedObservation(_))
    }
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
            Self::Control(source) => Some(source),
            Self::Observation(source) | Self::CommittedObservation(source) => Some(source),
            Self::StaleEpoch { .. } => None,
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
    /// Checked DMA memory access failed.
    Memory(MemoryAccessError),
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
            Self::Memory(source) => Some(source),
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
impl From<MemoryAccessError> for PdmaRegisterPortError {
    fn from(source: MemoryAccessError) -> Self {
        Self::Memory(source)
    }
}
impl From<PdmaRegisterPortError> for crate::semantic::PortError {
    fn from(source: PdmaRegisterPortError) -> Self {
        use crate::semantic::{PortBackendError, PortBackendKind};

        match source {
            PdmaRegisterPortError::Cpu(source) => Self::Cpu(source),
            PdmaRegisterPortError::Semantic(source) => source,
            PdmaRegisterPortError::Provisioning(source) => match source {
                PdmaProvisioningError::Provisioning(
                    PdmaProvisioningCause::AddressExhausted
                    | PdmaProvisioningCause::StorageExhausted,
                )
                | PdmaProvisioningError::LengthOutOfRange => Self::AllocationExhausted,
                source @ (PdmaProvisioningError::Provisioning(
                    PdmaProvisioningCause::Unmapped
                    | PdmaProvisioningCause::CrossesRegion
                    | PdmaProvisioningCause::OverlappingRegion
                    | PdmaProvisioningCause::InvalidRegion,
                )
                | PdmaProvisioningError::Access(_)
                | PdmaProvisioningError::InvalidProjection) => {
                    Self::Backend(PortBackendError::new(PortBackendKind::Provisioning, source))
                }
            },
            PdmaRegisterPortError::Register(source) => match source {
                PdmaRegisterAccessError::Control(source) => source,
                PdmaRegisterAccessError::StaleEpoch { expected, actual } => {
                    Self::StaleEpoch { expected, actual }
                }
                source @ (PdmaRegisterAccessError::Access(_)
                | PdmaRegisterAccessError::Observation(_)
                | PdmaRegisterAccessError::CommittedObservation(_)) => {
                    Self::Backend(PortBackendError::new(PortBackendKind::Register, source))
                }
            },
            PdmaRegisterPortError::Memory(source) => {
                Self::Backend(PortBackendError::new(PortBackendKind::Memory, source))
            }
        }
    }
}
