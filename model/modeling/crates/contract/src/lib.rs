//! Minimal PDMA wire records and register layout.

mod management;
mod record;
mod registers;
mod ring;

pub use management::*;
pub use record::*;
pub use registers::*;
pub use ring::{Ring, RingError, RingPointers};
