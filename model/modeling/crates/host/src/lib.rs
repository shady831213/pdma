//! Software channel driver, with PDMA runtime transport in [`runtime`].

mod channel;
mod error;
pub mod firmware;
mod management;
mod ring;
pub mod runtime;

pub use channel::{Channel, ChannelConfig, Rings};
pub use error::Error;
pub use management::{Management, ManagementRings};
pub use ring::RingConfig;
