//! Minimal PDMA wire records and register layout.

mod record;
mod registers;
mod ring;

pub use record::*;
pub use registers::*;
pub use ring::{Ring, RingError, RingPointers};

/// Maximum payload copied by a single command.
pub const MAX_COPY_BYTES: usize = 256;
