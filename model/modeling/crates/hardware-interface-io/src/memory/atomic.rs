use std::{error::Error, fmt};

/// Ordering of an atomic read within the backend's synchronization domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicReadOrder {
    /// No ordering beyond the atomic access itself.
    Relaxed,
    /// Acquire preceding publications observed by the read.
    Acquire,
}

/// Ordering of an atomic write within the backend's synchronization domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicWriteOrder {
    /// No ordering beyond the atomic access itself.
    Relaxed,
    /// Publish preceding accesses before this write becomes observable.
    Release,
}

/// Ordering of an indivisible read-modify-write operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicOrder {
    /// No additional ordering of surrounding accesses.
    Relaxed,
    /// Acquire publications observed by the read.
    Acquire,
    /// Release preceding accesses with the write.
    Release,
    /// Both acquire and release effects apply.
    AcquireRelease,
}

/// A fixed-width integer transformation; arithmetic wraps at the operand width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicUpdate<T> {
    /// Add the operand.
    Add(T),
    /// Subtract the operand.
    Subtract(T),
    /// Bitwise conjunction.
    And(T),
    /// Bitwise disjunction.
    Or(T),
    /// Bitwise exclusive disjunction.
    Xor(T),
    /// Replace the value.
    Exchange(T),
    /// Replace only if the current bits match; a mismatch performs no write.
    CompareExchange {
        /// Expected current bits.
        current: T,
        /// Replacement bits.
        new: T,
    },
}

/// One naturally aligned little-endian word access, returning the observed old bits.
///
/// Backends must serialize the whole access against overlapping ordinary and atomic
/// accesses. Errors occur before modification. Non-relaxed operations participate
/// in a common synchronization order within the supported domain; platform adapters
/// must establish the required visibility, not merely insert a host-language fence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicOperation<T> {
    /// Observe without modifying storage.
    Load(AtomicReadOrder),
    /// Replace storage, with a legal store ordering.
    Store {
        /// Replacement bits.
        value: T,
        /// Store ordering.
        order: AtomicWriteOrder,
    },
    /// Transform storage at one indivisible boundary.
    Update {
        /// Transformation to apply.
        update: AtomicUpdate<T>,
        /// Read-modify-write ordering.
        order: AtomicOrder,
    },
}

macro_rules! word_operation {
    ($word:ty) => {
        impl AtomicOperation<$word> {
            /// Computes the replacement word, or no write for a load or failed comparison.
            pub const fn replacement(self, old: $word) -> Option<$word> {
                match self {
                    Self::Load(_) => None,
                    Self::Store { value, .. } => Some(value),
                    Self::Update { update, .. } => match update {
                        AtomicUpdate::Add(value) => Some(old.wrapping_add(value)),
                        AtomicUpdate::Subtract(value) => Some(old.wrapping_sub(value)),
                        AtomicUpdate::And(value) => Some(old & value),
                        AtomicUpdate::Or(value) => Some(old | value),
                        AtomicUpdate::Xor(value) => Some(old ^ value),
                        AtomicUpdate::Exchange(value) => Some(value),
                        AtomicUpdate::CompareExchange { current, new } => {
                            if old == current {
                                Some(new)
                            } else {
                                None
                            }
                        }
                    },
                }
            }
        }
    };
}
word_operation!(u32);
word_operation!(u64);

/// Capability rejection or an unchanged backend-specific access failure.
#[derive(Debug, PartialEq, Eq)]
pub enum AtomicAccessError<E> {
    /// This memory capability does not implement indivisible word accesses.
    Unsupported,
    /// Access failed before any atomic modification.
    Access(E),
}

impl<E: fmt::Display> fmt::Display for AtomicAccessError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str("atomic memory access is unsupported"),
            Self::Access(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for AtomicAccessError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unsupported => None,
            Self::Access(error) => Some(error),
        }
    }
}
