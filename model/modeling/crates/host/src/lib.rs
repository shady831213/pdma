//! Software operations for the minimal DMA channel.

mod channel;
mod error;
mod model;
mod ring;

pub use channel::{Channel, Rings};
pub use error::Error;
pub use model::{Irq, Model};
pub use ring::RingConfig;
