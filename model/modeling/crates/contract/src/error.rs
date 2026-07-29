//! Shared typed errors for checked model contract operations.

use crate::abi::DeviceEpoch;
use crate::ids::{BufferId, CompletionRingId, QueueId};
use crate::irq::{IrqSource, IrqVector};
use core::fmt;

/// Result type used throughout the PDMA model.
pub type ModelResult<T> = Result<T, ModelError>;

/// Typed error set for checked model operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelError {
    /// A resettable device capability belongs to another generation.
    StaleDeviceEpoch {
        /// Device generation required at the commit boundary.
        expected: DeviceEpoch,
        /// Device generation carried by the supplied capability.
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
    /// An active IRQ source cannot change its delivery route.
    IrqSourceActive(IrqSource),
    /// The host IRQ delivery registry already has a handler for this vector.
    DuplicateIrqVector(IrqVector),
    /// The host IRQ delivery registry has no handler for this vector.
    UnregisteredIrqVector(IrqVector),
    /// Software completed deferred delivery registration without any vector handler.
    EmptyIrqHandlerRegistry,
    /// Deferred host IRQ delivery lost its software or harness endpoint before publication.
    IrqDeliveryUnavailable,
    /// CPU-local state was accessed outside modeled software CPU execution.
    SoftwareCpuInactive,
    /// CPU-local state belongs to another modeled software CPU.
    SoftwareCpuMismatch,
    /// CPU-local state was already borrowed during modeled CPU execution.
    SoftwareCpuStateBusy,
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
    /// A DMA object cannot fit in the modeled address space or backing storage.
    DmaAllocationExhausted,
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
            Self::StaleDeviceEpoch { expected, actual } => {
                write!(
                    f,
                    "stale device epoch {}, expected {}",
                    actual.raw(),
                    expected.raw()
                )
            }
            Self::ZeroCapacity => f.write_str("ring capacity must be greater than zero"),
            Self::RingFull => f.write_str("ring is full"),
            Self::UnknownBuffer(id) => write!(f, "unknown buffer {}", id.raw()),
            Self::UnknownQueue(id) => write!(f, "unknown queue {}", id.raw()),
            Self::UnknownCompletionRing(id) => write!(f, "unknown completion ring {}", id.raw()),
            Self::UnknownIrqSource(source) => write!(f, "unknown IRQ source {source:?}"),
            Self::IrqSourceActive(source) => write!(f, "IRQ source {source:?} is active"),
            Self::DuplicateIrqVector(vector) => {
                write!(f, "IRQ vector {} already has a handler", vector.raw())
            }
            Self::UnregisteredIrqVector(vector) => {
                write!(f, "IRQ vector {} has no registered handler", vector.raw())
            }
            Self::EmptyIrqHandlerRegistry => f.write_str("IRQ handler registry must not be empty"),
            Self::IrqDeliveryUnavailable => {
                f.write_str("deferred host IRQ delivery is unavailable")
            }
            Self::SoftwareCpuInactive => f.write_str("software CPU context is inactive"),
            Self::SoftwareCpuMismatch => {
                f.write_str("software CPU context does not match this state")
            }
            Self::SoftwareCpuStateBusy => f.write_str("software CPU-local state is busy"),
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
            Self::DmaAllocationExhausted => f.write_str("DMA allocation is exhausted"),
            Self::DriverQueueFull => f.write_str("driver command ring shadow is full"),
            Self::StepBudgetExhausted { max_steps } => {
                write!(f, "device still has work after {max_steps} steps")
            }
        }
    }
}

impl std::error::Error for ModelError {}
