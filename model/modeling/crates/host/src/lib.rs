//! Software channel driver, with PDMA runtime transport in [`runtime`].

mod channel;
mod error;
mod ring;
pub mod runtime;

pub use channel::{Channel, Rings};
pub use error::Error;
pub use ring::RingConfig;
