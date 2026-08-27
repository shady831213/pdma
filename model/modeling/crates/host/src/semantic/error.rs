//! Canonical host driver core surface.

use crate::register::{
    PdmaBatchError, PdmaClaimError, PdmaProvisioningError, PdmaRegisterAccessError,
    PdmaRegisterPortError,
};
use hardware_interface_runtime::{CpuContextError, MemoryAccessError};
use pdma_model_contract::{
    abi::mmio::ResponseCodecError,
    semantic::{BufferId, CompletionRingId, DeviceEpoch, IrqSource, QueueId},
};
use std::{error::Error, fmt};

/// Result returned by a semantic host port.
pub type PortResult<T> = Result<T, PortError>;

/// Result returned by CPU-bound or manual driver operations.
pub type DriverResult<T> = Result<T, DriverError>;

/// Device-port rejection projected entirely into host/runtime/contract types.
#[derive(Debug)]
pub enum PortError {
    /// A resettable capability belongs to another device epoch.
    StaleEpoch {
        /// Required device epoch.
        expected: DeviceEpoch,
        /// Supplied device epoch.
        actual: DeviceEpoch,
    },
    /// A zero-capacity ring was requested.
    ZeroCapacity,
    /// A ring has no producer space.
    RingFull,
    /// A buffer capability is unknown.
    UnknownBuffer(BufferId),
    /// A queue capability is unknown.
    UnknownQueue(QueueId),
    /// A completion-ring capability is unknown.
    UnknownCompletionRing(CompletionRingId),
    /// An interrupt source is unknown.
    UnknownIrqSource(IrqSource),
    /// An active interrupt source cannot change route.
    IrqSourceActive(IrqSource),
    /// A checked range overflowed.
    RangeOverflow,
    /// A checked buffer access exceeded its extent.
    OutOfBounds {
        /// Buffer capability checked.
        buffer: BufferId,
        /// Buffer extent.
        len: usize,
        /// Requested exclusive end.
        end: usize,
    },
    /// Memory provisioning exhausted its address space.
    AllocationExhausted,
    /// Submission state was inconsistent.
    SubmissionState,
    /// A finite compatibility step budget expired.
    StepBudgetExhausted {
        /// Supplied finite step budget.
        max_steps: usize,
    },
    /// The canonical port was polled outside its owning CPU.
    Cpu(CpuContextError),
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
    /// A generated response rejected bytes or a value.
    Response(ResponseCodecError),
}

impl PartialEq for PortError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::StaleEpoch {
                    expected: left_expected,
                    actual: left_actual,
                },
                Self::StaleEpoch {
                    expected: right_expected,
                    actual: right_actual,
                },
            ) => left_expected == right_expected && left_actual == right_actual,
            (Self::UnknownBuffer(left), Self::UnknownBuffer(right)) => left == right,
            (Self::UnknownQueue(left), Self::UnknownQueue(right)) => left == right,
            (Self::UnknownCompletionRing(left), Self::UnknownCompletionRing(right)) => {
                left == right
            }
            (Self::UnknownIrqSource(left), Self::UnknownIrqSource(right))
            | (Self::IrqSourceActive(left), Self::IrqSourceActive(right)) => left == right,
            (
                Self::OutOfBounds {
                    buffer: left_buffer,
                    len: left_len,
                    end: left_end,
                },
                Self::OutOfBounds {
                    buffer: right_buffer,
                    len: right_len,
                    end: right_end,
                },
            ) => left_buffer == right_buffer && left_len == right_len && left_end == right_end,
            (
                Self::StepBudgetExhausted { max_steps: left },
                Self::StepBudgetExhausted { max_steps: right },
            ) => left == right,
            (Self::Cpu(left), Self::Cpu(right)) => left == right,
            (Self::Provisioning(left), Self::Provisioning(right)) => left == right,
            (Self::Register(left), Self::Register(right)) => left == right,
            (Self::Batch(left), Self::Batch(right)) => left == right,
            (Self::Claim(left), Self::Claim(right)) => left == right,
            (Self::Memory(left), Self::Memory(right)) => left == right,
            (
                Self::Response(ResponseCodecError::Map(left)),
                Self::Response(ResponseCodecError::Map(right)),
            ) => left == right,
            (
                Self::Response(ResponseCodecError::Register(left)),
                Self::Response(ResponseCodecError::Register(right)),
            ) => left == right,
            (Self::ZeroCapacity, Self::ZeroCapacity)
            | (Self::RingFull, Self::RingFull)
            | (Self::RangeOverflow, Self::RangeOverflow)
            | (Self::AllocationExhausted, Self::AllocationExhausted)
            | (Self::SubmissionState, Self::SubmissionState) => true,
            _ => false,
        }
    }
}

impl Eq for PortError {}

impl From<PdmaRegisterPortError> for PortError {
    fn from(source: PdmaRegisterPortError) -> Self {
        match source {
            PdmaRegisterPortError::Cpu(source) => Self::Cpu(source),
            PdmaRegisterPortError::Semantic(source) => source,
            PdmaRegisterPortError::Provisioning(source) => Self::Provisioning(source),
            PdmaRegisterPortError::Register(source) => Self::Register(source),
            PdmaRegisterPortError::Batch(source) => Self::Batch(source),
            PdmaRegisterPortError::Claim(source) => Self::Claim(source),
            PdmaRegisterPortError::Memory(source) => Self::Memory(source),
            PdmaRegisterPortError::Response(source) => Self::Response(source),
        }
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "host port rejected operation: {self:?}")
    }
}

impl Error for PortError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(source) => Some(source),
            Self::Provisioning(source) => Some(source),
            Self::Register(source) => Some(source),
            Self::Batch(source) => Some(source),
            Self::Claim(source) => Some(source),
            Self::Memory(source) => Some(source),
            Self::Response(source) => Some(source),
            Self::StaleEpoch { .. }
            | Self::ZeroCapacity
            | Self::RingFull
            | Self::UnknownBuffer(_)
            | Self::UnknownQueue(_)
            | Self::UnknownCompletionRing(_)
            | Self::UnknownIrqSource(_)
            | Self::IrqSourceActive(_)
            | Self::RangeOverflow
            | Self::OutOfBounds { .. }
            | Self::AllocationExhausted
            | Self::SubmissionState
            | Self::StepBudgetExhausted { .. } => None,
        }
    }
}

/// Failure from driver ownership, accounting, or a supplied port.
#[derive(Debug, PartialEq, Eq)]
pub enum DriverError {
    /// No software CPU is active for this poll.
    InactiveCpu(CpuContextError),
    /// A different software CPU is active for this poll.
    WrongCpu(CpuContextError),
    /// CPU-local accounting is already borrowed.
    StateBusy(CpuContextError),
    /// Driver-local command credit is exhausted.
    QueueFull,
    /// The supplied port rejected the operation.
    Port(PortError),
}

impl From<CpuContextError> for DriverError {
    fn from(source: CpuContextError) -> Self {
        match source {
            CpuContextError::Inactive => Self::InactiveCpu(source),
            CpuContextError::Mismatch => Self::WrongCpu(source),
            CpuContextError::StateBusy => Self::StateBusy(source),
        }
    }
}

impl From<PortError> for DriverError {
    fn from(source: PortError) -> Self {
        Self::Port(source)
    }
}

impl fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InactiveCpu(_) => formatter.write_str("driver poll has no active CPU"),
            Self::WrongCpu(_) => formatter.write_str("driver poll is running on the wrong CPU"),
            Self::StateBusy(_) => formatter.write_str("driver accounting state is busy"),
            Self::QueueFull => formatter.write_str("driver command credit is exhausted"),
            Self::Port(_) => formatter.write_str("driver port operation failed"),
        }
    }
}

impl Error for DriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InactiveCpu(source) | Self::WrongCpu(source) | Self::StateBusy(source) => {
                Some(source)
            }
            Self::Port(source) => Some(source),
            Self::QueueFull => None,
        }
    }
}
