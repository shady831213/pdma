//! Minimal PDMA wire records and register layout.

mod control;
mod management;
mod record;
mod registers;
mod ring;
mod view;

pub use control::*;
pub use management::*;
pub use record::*;
pub use registers::*;
pub use ring::{Ring, RingError, RingPointers};
pub use view::RegisterView;
