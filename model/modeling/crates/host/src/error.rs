use core::fmt;
use pdma_model_contract::{BufferId, CompletionRingId, DeviceEpoch, IrqSource, IrqVector, QueueId};
use pdma_model_device::ModelError as DeviceModelError;

/// Result returned by legacy host-model operations.
pub type ModelResult<T> = Result<T, ModelError>;

/// Temporary leaf-compatible host error projection retained until Task 26.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelError {
    /// A resettable device capability belongs to another generation.
    StaleDeviceEpoch {
        /// Required generation.
        expected: DeviceEpoch,
        /// Supplied generation.
        actual: DeviceEpoch,
    },
    /// A ring capacity of zero was requested.
    ZeroCapacity,
    /// A producer attempted to push into a full ring.
    RingFull,
    /// The requested buffer does not exist.
    UnknownBuffer(BufferId),
    /// The requested queue does not exist.
    UnknownQueue(QueueId),
    /// The requested completion ring does not exist.
    UnknownCompletionRing(CompletionRingId),
    /// The requested IRQ source does not exist.
    UnknownIrqSource(IrqSource),
    /// An active IRQ source cannot change route.
    IrqSourceActive(IrqSource),
    /// The host registry already has a handler for the vector.
    DuplicateIrqVector(IrqVector),
    /// The host registry has no handler for the vector.
    UnregisteredIrqVector(IrqVector),
    /// Deferred registration completed without handlers.
    EmptyIrqHandlerRegistry,
    /// Deferred IRQ delivery cannot publish to software.
    IrqDeliveryUnavailable,
    /// No modeled software CPU is active.
    SoftwareCpuInactive,
    /// The active CPU does not own the state.
    SoftwareCpuMismatch,
    /// CPU-local state is already borrowed.
    SoftwareCpuStateBusy,
    /// A checked range calculation overflowed.
    RangeOverflow,
    /// A checked access exceeded the buffer.
    OutOfBounds {
        /// Buffer checked.
        buffer: BufferId,
        /// Buffer length.
        len: usize,
        /// Requested exclusive end.
        end: usize,
    },
    /// DMA address space or backing reservation was exhausted.
    DmaAllocationExhausted,
    /// DMA submission state was inconsistent.
    DmaSubmissionState,
    /// Driver-local command credit is exhausted.
    DriverQueueFull,
    /// Runnable work remains after a finite manual-run budget.
    StepBudgetExhausted {
        /// Supplied step budget.
        max_steps: usize,
    },
}

impl From<DeviceModelError> for ModelError {
    fn from(error: DeviceModelError) -> Self {
        match error {
            DeviceModelError::StaleDeviceEpoch { expected, actual } => {
                Self::StaleDeviceEpoch { expected, actual }
            }
            DeviceModelError::ZeroCapacity => Self::ZeroCapacity,
            DeviceModelError::RingFull => Self::RingFull,
            DeviceModelError::UnknownBuffer(id) => Self::UnknownBuffer(id),
            DeviceModelError::UnknownQueue(id) => Self::UnknownQueue(id),
            DeviceModelError::UnknownCompletionRing(id) => Self::UnknownCompletionRing(id),
            DeviceModelError::UnknownIrqSource(source) => Self::UnknownIrqSource(source),
            DeviceModelError::IrqSourceActive(source) => Self::IrqSourceActive(source),
            DeviceModelError::RangeOverflow => Self::RangeOverflow,
            DeviceModelError::OutOfBounds { buffer, len, end } => {
                Self::OutOfBounds { buffer, len, end }
            }
            DeviceModelError::DmaAllocationExhausted => Self::DmaAllocationExhausted,
            DeviceModelError::DmaSubmissionState => Self::DmaSubmissionState,
            DeviceModelError::StepBudgetExhausted { max_steps } => {
                Self::StepBudgetExhausted { max_steps }
            }
        }
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleDeviceEpoch { expected, actual } => write!(
                formatter,
                "stale device epoch {}, expected {}",
                actual.raw(),
                expected.raw()
            ),
            Self::ZeroCapacity => formatter.write_str("ring capacity must be greater than zero"),
            Self::RingFull => formatter.write_str("ring is full"),
            Self::UnknownBuffer(id) => write!(formatter, "unknown buffer {}", id.raw()),
            Self::UnknownQueue(id) => write!(formatter, "unknown queue {}", id.raw()),
            Self::UnknownCompletionRing(id) => {
                write!(formatter, "unknown completion ring {}", id.raw())
            }
            Self::UnknownIrqSource(source) => write!(formatter, "unknown IRQ source {source:?}"),
            Self::IrqSourceActive(source) => write!(formatter, "IRQ source {source:?} is active"),
            Self::DuplicateIrqVector(vector) => write!(
                formatter,
                "IRQ vector {} already has a handler",
                vector.raw()
            ),
            Self::UnregisteredIrqVector(vector) => write!(
                formatter,
                "IRQ vector {} has no registered handler",
                vector.raw()
            ),
            Self::EmptyIrqHandlerRegistry => {
                formatter.write_str("IRQ handler registry must not be empty")
            }
            Self::IrqDeliveryUnavailable => {
                formatter.write_str("deferred host IRQ delivery is unavailable")
            }
            Self::SoftwareCpuInactive => formatter.write_str("software CPU context is inactive"),
            Self::SoftwareCpuMismatch => {
                formatter.write_str("software CPU context does not match this state")
            }
            Self::SoftwareCpuStateBusy => formatter.write_str("software CPU-local state is busy"),
            Self::RangeOverflow => formatter.write_str("range calculation overflowed"),
            Self::OutOfBounds { buffer, len, end } => write!(
                formatter,
                "buffer {} range end {} exceeds length {}",
                buffer.raw(),
                end,
                len
            ),
            Self::DmaAllocationExhausted => formatter.write_str("DMA allocation is exhausted"),
            Self::DmaSubmissionState => formatter.write_str("DMA submission state is inconsistent"),
            Self::DriverQueueFull => formatter.write_str("driver command ring shadow is full"),
            Self::StepBudgetExhausted { max_steps } => {
                write!(formatter, "device still has work after {max_steps} steps")
            }
        }
    }
}

impl std::error::Error for ModelError {}
