//! Software channel driver, with PDMA runtime transport in [`runtime`].

mod error;
pub mod firmware;
mod management;
mod ring;
pub mod runtime;

pub use error::Error;
pub use management::{Management, ManagementConfig, ManagementRings};
pub use pdma_model_software::dma::{Channel, ChannelConfig, Error as DmaError, Rings};
pub use ring::RingConfig;
