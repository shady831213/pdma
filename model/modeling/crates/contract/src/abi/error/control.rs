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
    /// A control request contains feature bits not assigned by ABI v1.
    UnknownFeatureBits {
        /// Unassigned bits after masking known features.
        bits: u64,
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
        /// Supplied base IOVA.
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
    /// Consumer shadow is ahead of producer shadow.
    CursorRegression {
        /// Supplied producer shadow.
        producer: u64,
        /// Supplied consumer shadow.
        consumer: u64,
    },
    /// A proposed shadow regresses its previously published value.
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
    /// A queue activation assigned the wrong role to a ring layout.
    UnexpectedRingRecord {
        /// Required record width for the role.
        expected: u16,
        /// Actual record width.
        actual: u16,
    },
    /// A queue ring depth differs from its semantic configuration.
    QueueDepthMismatch {
        /// Ring role.
        ring: &'static str,
        /// Configured depth.
        expected: usize,
        /// Explicit activation depth.
        actual: u32,
    },
    /// Queue-local event depth differs from completion depth.
    EventDepthMismatch {
        /// Derived completion depth.
        expected: usize,
        /// Explicit event depth.
        actual: u32,
    },
    /// Whole-device generation cannot advance without wrapping.
    DeviceEpochOverflow {
        /// Current generation.
        current: u64,
    },
    /// A typed status response contains an unassigned state value.
    UnknownStatusValue {
        /// Unassigned value.
        value: u16,
    },
}

impl fmt::Display for AbiControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { requested } => {
                write!(f, "ABI version {requested} is not supported")
            }
            Self::UnknownFeatureBits { bits } => {
                write!(
                    f,
                    "control request contains unknown ABI feature bits {bits:#x}"
                )
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
            Self::CursorUpdateRegression { current, proposed } => {
                write!(f, "cursor update regresses {current} to {proposed}")
            }
            Self::CursorDistanceExceeded {
                producer,
                consumer,
                depth,
            } => write!(
                f,
                "cursor distance {producer} - {consumer} exceeds ring depth {depth}"
            ),
            Self::CursorIncrementOverflow { cursor, increment } => {
                write!(f, "cursor {cursor} + {increment} overflows u64")
            }
            Self::UnexpectedRingRecord { expected, actual } => {
                write!(
                    f,
                    "ring role requires {expected}-byte records, not {actual}"
                )
            }
            Self::QueueDepthMismatch {
                ring,
                expected,
                actual,
            } => write!(
                f,
                "{ring} ring depth must be configured as {expected}, not {actual}"
            ),
            Self::EventDepthMismatch { expected, actual } => {
                write!(f, "event ring depth must be {expected}, not {actual}")
            }
            Self::DeviceEpochOverflow { current } => {
                write!(f, "device epoch {current} cannot advance without wrapping")
            }
            Self::UnknownStatusValue { value } => {
                write!(f, "device status value {value} is not assigned by ABI v1")
            }
        }
    }
}

impl std::error::Error for AbiControlError {}
