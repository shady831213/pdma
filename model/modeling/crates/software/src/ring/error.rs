use hardware_interface_memory::MemoryAccessError;
use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::ring::RingError;
use std::{error::Error, fmt};

/// Typed software ring failures, with retained operations available for retry.
#[derive(Clone, Copy, Debug)]
pub enum SoftRingError<E, M = MemoryAccessError> {
    /// Cursor transport failed.
    Cursor(E),
    /// Entry memory access failed.
    Memory(M),
    /// Entry encoding or decoding failed.
    Codec(SchemaCodecError),
    /// Ring geometry or cursor movement is invalid.
    Ring(RingError),
    /// A prior push must be resumed first.
    Busy,
    /// No retained operation is available.
    Idle,
    /// An endpoint-owned cursor moved outside the retained operation.
    CursorChanged,
}

impl<E: fmt::Display, M: fmt::Display> fmt::Display for SoftRingError<E, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cursor(error) => fmt::Display::fmt(error, f),
            Self::Memory(error) => fmt::Display::fmt(error, f),
            Self::Codec(error) => error.fmt(f),
            Self::Ring(error) => error.fmt(f),
            Self::Busy => f.write_str("software ring push is pending"),
            Self::Idle => f.write_str("software ring has no retained operation"),
            Self::CursorChanged => f.write_str("software ring cursor changed unexpectedly"),
        }
    }
}
impl<E: Error + 'static, M: Error + 'static> Error for SoftRingError<E, M> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cursor(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Ring(error) => Some(error),
            Self::Busy | Self::Idle | Self::CursorChanged => None,
        }
    }
}
