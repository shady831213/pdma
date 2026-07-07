//! Shared typed errors for checked model contract operations.

use crate::ids::{BufferId, CompletionRingId, QueueId};
use crate::irq::IrqSource;
use core::fmt;

/// Result type used throughout the PDMA model.
pub type ModelResult<T> = Result<T, ModelError>;

/// Typed error set for checked model operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelError {
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
    /// A checked offset plus length calculation overflowed.
    RangeOverflow,
    /// A memory read or write exceeded the buffer bounds.
    OutOfBounds {
        /// Buffer whose range was checked.
        buffer: BufferId,
        /// Buffer length in bytes.
        len: usize,
        /// Requested exclusive range end.
        end: usize,
    },
    /// The driver has no locally reclaimed command slots.
    DriverQueueFull,
    /// The device still had runnable work after the caller's step budget expired.
    StepBudgetExhausted {
        /// Step budget supplied by the caller.
        max_steps: usize,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => f.write_str("ring capacity must be greater than zero"),
            Self::RingFull => f.write_str("ring is full"),
            Self::UnknownBuffer(id) => write!(f, "unknown buffer {}", id.raw()),
            Self::UnknownQueue(id) => write!(f, "unknown queue {}", id.raw()),
            Self::UnknownCompletionRing(id) => write!(f, "unknown completion ring {}", id.raw()),
            Self::UnknownIrqSource(source) => write!(f, "unknown IRQ source {source:?}"),
            Self::RangeOverflow => f.write_str("range calculation overflowed"),
            Self::OutOfBounds { buffer, len, end } => {
                write!(
                    f,
                    "buffer {} range end {} exceeds length {}",
                    buffer.raw(),
                    end,
                    len
                )
            }
            Self::DriverQueueFull => f.write_str("driver command ring shadow is full"),
            Self::StepBudgetExhausted { max_steps } => {
                write!(f, "device still has work after {max_steps} steps")
            }
        }
    }
}

impl std::error::Error for ModelError {}
