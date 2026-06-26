//! Step-based PDMA v0 conceptual model.
//!
//! The crate intentionally starts with memory-to-memory copy vertical slices:
//! driver submission, command ring, doorbell/runnable queue, scheduler lowering,
//! deterministic copy execution, completion publication, and driver polling.

pub mod arbitration;
pub mod completion;
pub mod desc;
pub mod device;
pub mod driver;
pub mod engines;
pub mod event;
pub mod ids;
pub mod memory;
pub mod queue;
pub mod rings;
pub mod scheduler;

pub use arbitration::{ArbitrationPolicy, QueueArbiter};
pub use completion::{CompletionRecord, CompletionStatus};
pub use desc::{CopyRequest, ExternalCommand, ScheduledCopy, StridedCopyRequest};
pub use device::{PdmaDevice, RunSummary};
pub use driver::Driver;
pub use event::{EventErrorKind, EventRecord, MemoryFault};
pub use ids::{BufferId, CommandId, CompletionRingId, QueueId};
pub use memory::Memory;
pub use queue::{
    EventRouting, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, SoftwareOwnerId,
};
pub use rings::Ring;

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
            Self::UnknownCompletionRing(id) => {
                write!(f, "unknown completion ring {}", id.raw())
            }
            Self::RangeOverflow => f.write_str("range calculation overflowed"),
            Self::OutOfBounds { buffer, len, end } => write!(
                f,
                "buffer {} range end {} exceeds length {}",
                buffer.raw(),
                end,
                len
            ),
            Self::DriverQueueFull => f.write_str("driver command ring shadow is full"),
            Self::StepBudgetExhausted { max_steps } => {
                write!(f, "device still has work after {max_steps} steps")
            }
        }
    }
}

impl std::error::Error for ModelError {}
