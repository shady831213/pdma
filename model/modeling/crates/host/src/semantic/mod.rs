//! Host-owned semantic driver API independent of exact register transport.

mod driver;
mod error;
mod port;

pub use driver::{AsyncDriver, DriverIrqRecord, ManualDriver};
pub use error::{DriverError, DriverResult, PortError, PortResult};
pub use port::DriverPort;
