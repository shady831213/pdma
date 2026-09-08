//! Typed failures at ABI control boundaries.

use core::fmt;

/// Failure while validating a typed ABI control request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiControlError {
    /// The requested ABI version is not supported.
    UnsupportedVersion {
        /// Requested ABI version.
        requested: u16,
    },
    /// A queue-ring depth is zero or exceeds the ABI-v1 limit.
    InvalidRingDepth {
        /// Supplied depth.
        depth: u64,
        /// Largest accepted depth.
        maximum: u32,
    },
    /// Multiplying depth by record width overflowed.
    RingExtentOverflow {
        /// Supplied depth.
        depth: u64,
        /// Canonical record width.
        record_size: u16,
    },
    /// A ring base does not meet its record alignment.
    MisalignedRing {
        /// Supplied base DMA address.
        base: u64,
        /// Required alignment.
        alignment: u64,
    },
    /// A ring region does not have its exact canonical extent.
    RingExtentMismatch {
        /// Required byte extent.
        expected: u64,
        /// Supplied byte extent.
        actual: u64,
    },
    /// A consumer cursor is ahead of its producer cursor.
    CursorRegression {
        /// Supplied producer cursor.
        producer: u64,
        /// Supplied consumer cursor.
        consumer: u64,
    },
    /// A producer-to-consumer distance exceeds the ring depth.
    CursorDistanceExceeded {
        /// Supplied producer cursor.
        producer: u64,
        /// Supplied consumer cursor.
        consumer: u64,
        /// Configured ring depth.
        depth: u32,
    },
    /// A cursor publication moves backward.
    CursorUpdateRegression {
        /// Current cursor.
        current: u64,
        /// Proposed cursor.
        proposed: u64,
    },
    /// Advancing a cursor exceeds its fixed-width representation.
    CursorIncrementOverflow {
        /// Current cursor.
        cursor: u64,
        /// Requested increment.
        increment: u64,
    },
}

impl fmt::Display for AbiControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { requested } => {
                write!(f, "ABI version {requested} is not supported")
            }
            Self::InvalidRingDepth { depth, maximum } => {
                write!(f, "ring depth {depth} is outside 1..={maximum}")
            }
            Self::RingExtentOverflow { depth, record_size } => {
                write!(f, "ring extent {depth} * {record_size} overflows u64")
            }
            Self::MisalignedRing { base, alignment } => {
                write!(f, "ring base {base} is not aligned to {alignment}")
            }
            Self::RingExtentMismatch { expected, actual } => {
                write!(f, "ring extent must be {expected} bytes, not {actual}")
            }
            Self::CursorRegression { producer, consumer } => {
                write!(f, "producer {producer} is behind consumer {consumer}")
            }
            Self::CursorDistanceExceeded {
                producer,
                consumer,
                depth,
            } => write!(
                f,
                "cursor distance {producer} - {consumer} exceeds ring depth {depth}"
            ),
            Self::CursorUpdateRegression { current, proposed } => {
                write!(f, "cursor update regressed from {current} to {proposed}")
            }
            Self::CursorIncrementOverflow { cursor, increment } => {
                write!(f, "cursor {cursor} cannot advance by {increment}")
            }
        }
    }
}

impl std::error::Error for AbiControlError {}
