use core::fmt;

/// Result returned by pure semantic validation.
pub type SemanticResult<T> = Result<T, SemanticError>;

/// Failure owned by model-semantic contract behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticError {
    /// A ring capacity of zero was requested.
    ZeroCapacity,
    /// A producer attempted to push into a full ring.
    RingFull,
    /// A consumer shadow is ahead of its producer shadow.
    CursorRegression {
        /// Supplied producer shadow.
        producer: u64,
        /// Supplied consumer shadow.
        consumer: u64,
    },
    /// A proposed cursor regresses its published value.
    CursorUpdateRegression {
        /// Current published shadow.
        current: u64,
        /// Proposed shadow.
        proposed: u64,
    },
    /// Producer-to-consumer distance exceeds retained capacity.
    CursorDistanceExceeded {
        /// Supplied producer shadow.
        producer: u64,
        /// Supplied consumer shadow.
        consumer: u64,
        /// Retained ring capacity.
        depth: u32,
    },
    /// Advancing a monotonic cursor would wrap.
    CursorIncrementOverflow {
        /// Current shadow.
        cursor: u64,
        /// Requested increment.
        increment: u64,
    },
    /// Whole-device generation cannot advance without wrapping.
    DeviceEpochOverflow {
        /// Current generation.
        current: u64,
    },
}

impl fmt::Display for SemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => formatter.write_str("ring capacity must be greater than zero"),
            Self::RingFull => formatter.write_str("ring is full"),
            Self::CursorRegression { producer, consumer } => {
                write!(
                    formatter,
                    "producer {producer} is behind consumer {consumer}"
                )
            }
            Self::CursorUpdateRegression { current, proposed } => {
                write!(formatter, "cursor update regresses {current} to {proposed}")
            }
            Self::CursorDistanceExceeded {
                producer,
                consumer,
                depth,
            } => write!(
                formatter,
                "cursor distance {producer} - {consumer} exceeds ring depth {depth}"
            ),
            Self::CursorIncrementOverflow { cursor, increment } => {
                write!(formatter, "cursor {cursor} + {increment} overflows u64")
            }
            Self::DeviceEpochOverflow { current } => {
                write!(
                    formatter,
                    "device epoch {current} cannot advance without wrapping"
                )
            }
        }
    }
}

impl std::error::Error for SemanticError {}
