use core::convert::Infallible;
use core::fmt::{self, Display, Formatter};

/// Rejection of one register-window transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterAccessError<E = Infallible> {
    /// The address is outside this window; another window may accept it.
    OutsideWindow {
        /// Untranslated caller address.
        address: usize,
    },
    /// No leaf starts at this in-window offset.
    InvalidOffset {
        /// Window-relative byte offset.
        offset: usize,
    },
    /// The transaction width differs from the exact leaf width.
    WidthMismatch {
        /// Window-relative byte offset.
        offset: usize,
        /// Declared leaf width.
        expected: usize,
        /// Caller-provided width.
        actual: usize,
    },
    /// Software cannot read this leaf.
    ReadForbidden {
        /// Window-relative byte offset.
        offset: usize,
    },
    /// Software cannot write this leaf.
    WriteForbidden {
        /// Window-relative byte offset.
        offset: usize,
    },
    /// A write changes bits outside the writable mask.
    ReservedBitChange {
        /// Window-relative byte offset.
        offset: usize,
        /// Declared writable bits.
        writable_mask: u64,
        /// Changed non-writable bits.
        changed: u64,
    },
    /// Layout metadata or scalar access is inconsistent.
    InvalidLayout {
        /// Window-relative byte offset.
        offset: usize,
    },
    /// The owning hardware object rejected the transaction.
    Other(E),
}

impl<E> RegisterAccessError<E> {
    /// Transforms only the owning object's rejection.
    pub fn map_other<F>(self, map: impl FnOnce(E) -> F) -> RegisterAccessError<F> {
        match self {
            Self::OutsideWindow { address } => RegisterAccessError::OutsideWindow { address },
            Self::InvalidOffset { offset } => RegisterAccessError::InvalidOffset { offset },
            Self::WidthMismatch {
                offset,
                expected,
                actual,
            } => RegisterAccessError::WidthMismatch {
                offset,
                expected,
                actual,
            },
            Self::ReadForbidden { offset } => RegisterAccessError::ReadForbidden { offset },
            Self::WriteForbidden { offset } => RegisterAccessError::WriteForbidden { offset },
            Self::ReservedBitChange {
                offset,
                writable_mask,
                changed,
            } => RegisterAccessError::ReservedBitChange {
                offset,
                writable_mask,
                changed,
            },
            Self::InvalidLayout { offset } => RegisterAccessError::InvalidLayout { offset },
            Self::Other(source) => RegisterAccessError::Other(map(source)),
        }
    }
}

impl RegisterAccessError {
    /// Adds an application error type to a schema-only rejection.
    pub fn widen<E>(self) -> RegisterAccessError<E> {
        self.map_other(|never| match never {})
    }
}

impl<E: Display> Display for RegisterAccessError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideWindow { address } => {
                write!(formatter, "address {address} outside register window")
            }
            Self::InvalidOffset { offset } => write!(formatter, "no register at offset {offset}"),
            Self::WidthMismatch {
                offset,
                expected,
                actual,
            } => write!(
                formatter,
                "register at offset {offset} requires {expected} bytes, received {actual}"
            ),
            Self::ReadForbidden { offset } => {
                write!(formatter, "read forbidden at offset {offset}")
            }
            Self::WriteForbidden { offset } => {
                write!(formatter, "write forbidden at offset {offset}")
            }
            Self::ReservedBitChange {
                offset,
                writable_mask,
                changed,
            } => write!(
                formatter,
                "register at offset {offset} changes reserved bits {changed:#x} outside mask {writable_mask:#x}"
            ),
            Self::InvalidLayout { offset } => {
                write!(formatter, "invalid register layout at offset {offset}")
            }
            Self::Other(source) => Display::fmt(source, formatter),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for RegisterAccessError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Other(source) => Some(source),
            Self::OutsideWindow { .. }
            | Self::InvalidOffset { .. }
            | Self::WidthMismatch { .. }
            | Self::ReadForbidden { .. }
            | Self::WriteForbidden { .. }
            | Self::ReservedBitChange { .. }
            | Self::InvalidLayout { .. } => None,
        }
    }
}
