//! Minimal PDMA wire records and register layout.

mod record;
mod registers;
mod ring;

pub use record::*;
pub use registers::*;
pub use ring::{Ring, RingError, RingPointers};
