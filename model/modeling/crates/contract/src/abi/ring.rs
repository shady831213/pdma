//! Fixed-width ABI-v1 ring values.

use super::{
    AbiControlError, COMPLETION_ALIGNMENT, COMPLETION_SIZE, DESCRIPTOR_ALIGNMENT, DESCRIPTOR_SIZE,
    EVENT_ALIGNMENT, EVENT_SIZE,
};

/// Canonical ABI-v1 record carried by a queue ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingRecord {
    /// Host-produced command descriptor.
    Submission,
    /// Device-produced successful completion.
    Completion,
    /// Device-produced fault event.
    Event,
}

impl RingRecord {
    /// Canonical record width in bytes.
    pub const fn record_size(self) -> u16 {
        match self {
            Self::Submission => DESCRIPTOR_SIZE,
            Self::Completion => COMPLETION_SIZE,
            Self::Event => EVENT_SIZE,
        }
    }

    /// Required base-I/O-address alignment.
    pub const fn alignment(self) -> u64 {
        match self {
            Self::Submission => DESCRIPTOR_ALIGNMENT,
            Self::Completion => COMPLETION_ALIGNMENT,
            Self::Event => EVENT_ALIGNMENT,
        }
    }
}

/// One fixed-width monotonic queue cursor shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RingCursor(u64);

impl RingCursor {
    /// Creates a cursor from its complete monotonic shadow.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the complete monotonic shadow.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Advances without wrapping.
    pub const fn checked_increment(self, increment: u64) -> Result<Self, AbiControlError> {
        match self.0.checked_add(increment) {
            Some(next) => Ok(Self(next)),
            None => Err(AbiControlError::CursorIncrementOverflow {
                cursor: self.0,
                increment,
            }),
        }
    }
}
