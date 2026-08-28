use core::fmt;
use pdma_model_contract::semantic::{
    BufferId, CompletionRingId, DeviceEpoch, EventErrorKind, IrqSource, QueueId, SemanticError,
};

/// Result returned by semantic device-model operations.
pub type ModelResult<T> = Result<T, ModelError>;

/// Typed error projection for semantic device-model operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    /// A resettable capability belongs to another device generation.
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
    /// A checked offset calculation overflowed.
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
    /// Runnable work remains after a finite manual-run budget.
    StepBudgetExhausted {
        /// Supplied step budget.
        max_steps: usize,
    },
}

impl From<SemanticError> for ModelError {
    fn from(error: SemanticError) -> Self {
        match error {
            SemanticError::ZeroCapacity => Self::ZeroCapacity,
            SemanticError::RingFull => Self::RingFull,
            SemanticError::CursorRegression { .. }
            | SemanticError::CursorUpdateRegression { .. }
            | SemanticError::CursorDistanceExceeded { .. }
            | SemanticError::CursorIncrementOverflow { .. }
            | SemanticError::DeviceEpochOverflow { .. } => Self::DmaSubmissionState,
        }
    }
}

impl ModelError {
    pub(crate) const fn from_memory_fault(buffer: BufferId, error: EventErrorKind) -> Self {
        match error {
            EventErrorKind::UnknownBuffer => Self::UnknownBuffer(buffer),
            EventErrorKind::RangeOverflow => Self::RangeOverflow,
            EventErrorKind::OutOfBounds { len, end } => Self::OutOfBounds { buffer, len, end },
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
            Self::StepBudgetExhausted { max_steps } => {
                write!(formatter, "device still has work after {max_steps} steps")
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// Device execution failure independent of control, batch, and reset protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdmaRunError {
    /// The semantic compatibility executor returned a typed cause.
    Semantic(ModelError),
}

impl fmt::Display for PdmaRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Semantic(error) => write!(formatter, "PDMA execution failed: {error}"),
        }
    }
}

impl std::error::Error for PdmaRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Semantic(error) => Some(error),
        }
    }
}

impl From<ModelError> for PdmaRunError {
    fn from(error: ModelError) -> Self {
        Self::Semantic(error)
    }
}

/// Whole-device reset failure preserving its typed control cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdmaResetError {
    /// Reset was rejected by the canonical device-control layer.
    Control(crate::runner::PdmaControlError),
}

impl fmt::Display for PdmaResetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(error) => write!(formatter, "PDMA reset failed: {error}"),
        }
    }
}

impl std::error::Error for PdmaResetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Control(error) => Some(error),
        }
    }
}

impl From<crate::runner::PdmaControlError> for PdmaResetError {
    fn from(error: crate::runner::PdmaControlError) -> Self {
        Self::Control(error)
    }
}
