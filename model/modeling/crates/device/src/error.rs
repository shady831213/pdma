use core::fmt;
use pdma_model_contract::abi::AbiFault;
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, IrqSource, QueueId, SemanticError,
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
    /// A hardware address access or descriptor produced an exact ABI fault.
    MemoryFault(AbiFault),
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
    /// DMA submission state was inconsistent.
    DmaSubmissionState,
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
            Self::MemoryFault(fault) => write!(formatter, "hardware memory fault {fault:?}"),
            Self::UnknownQueue(id) => write!(formatter, "unknown queue {}", id.raw()),
            Self::UnknownCompletionRing(id) => {
                write!(formatter, "unknown completion ring {}", id.raw())
            }
            Self::UnknownIrqSource(source) => write!(formatter, "unknown IRQ source {source:?}"),
            Self::IrqSourceActive(source) => write!(formatter, "IRQ source {source:?} is active"),
            Self::RangeOverflow => formatter.write_str("range calculation overflowed"),
            Self::DmaSubmissionState => formatter.write_str("DMA submission state is inconsistent"),
        }
    }
}

impl std::error::Error for ModelError {}
