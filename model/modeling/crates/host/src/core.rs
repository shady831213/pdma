//! Canonical host driver core surface.

pub use crate::driver::{AsyncDriver, ManualDriver};
pub use crate::port::DriverPort;
use hardware_interface_runtime::CpuContextError;
pub use hardware_interface_runtime::{CpuLocal, SoftwareCpu};
use pdma_model_contract::{BufferId, CompletionRingId, DeviceEpoch, IrqSource, QueueId};
use std::{error::Error, fmt};

/// Device-port rejection projected entirely into host/runtime/contract types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl From<crate::ModelError> for PortError {
    fn from(source: crate::ModelError) -> Self {
        match source {
            crate::ModelError::StaleDeviceEpoch { expected, actual } => {
                Self::StaleEpoch { expected, actual }
            }
            crate::ModelError::ZeroCapacity => Self::ZeroCapacity,
            crate::ModelError::RingFull => Self::RingFull,
            crate::ModelError::UnknownBuffer(id) => Self::UnknownBuffer(id),
            crate::ModelError::UnknownQueue(id) => Self::UnknownQueue(id),
            crate::ModelError::UnknownCompletionRing(id) => Self::UnknownCompletionRing(id),
            crate::ModelError::UnknownIrqSource(source) => Self::UnknownIrqSource(source),
            crate::ModelError::IrqSourceActive(source) => Self::IrqSourceActive(source),
            crate::ModelError::RangeOverflow => Self::RangeOverflow,
            crate::ModelError::OutOfBounds { buffer, len, end } => {
                Self::OutOfBounds { buffer, len, end }
            }
            crate::ModelError::DmaAllocationExhausted => Self::AllocationExhausted,
            crate::ModelError::DmaSubmissionState
            | crate::ModelError::SoftwareCpuInactive
            | crate::ModelError::SoftwareCpuMismatch
            | crate::ModelError::SoftwareCpuStateBusy
            | crate::ModelError::DriverQueueFull
            | crate::ModelError::DuplicateIrqVector(_)
            | crate::ModelError::UnregisteredIrqVector(_)
            | crate::ModelError::EmptyIrqHandlerRegistry
            | crate::ModelError::IrqDeliveryUnavailable => Self::SubmissionState,
            crate::ModelError::StepBudgetExhausted { max_steps } => {
                Self::StepBudgetExhausted { max_steps }
            }
        }
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "host port rejected operation: {self:?}")
    }
}

impl Error for PortError {}

/// Failure from driver ownership, accounting, or a supplied port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl From<crate::ModelError> for DriverError {
    fn from(source: crate::ModelError) -> Self {
        match source {
            crate::ModelError::SoftwareCpuInactive => Self::InactiveCpu(CpuContextError::Inactive),
            crate::ModelError::SoftwareCpuMismatch => Self::WrongCpu(CpuContextError::Mismatch),
            crate::ModelError::SoftwareCpuStateBusy => Self::StateBusy(CpuContextError::StateBusy),
            crate::ModelError::DriverQueueFull => Self::QueueFull,
            source => Self::Port(source.into()),
        }
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
