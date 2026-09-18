//! Minimal PDMA wire records and register layout.

mod control;
mod dma;
mod management;
mod record;
mod register;
mod ring;
mod view;

pub use control::*;
pub use dma::*;
pub use management::*;
pub use record::*;
pub use register::*;
pub use ring::{Ring, RingError, RingPointers};
pub use view::RegisterView;
