//! Concrete management access-window hardware.

mod access_window;
mod error;
mod firmware_status;

pub use access_window::AccessWindow;
pub use error::{AccessError, WindowConfigError};
pub use firmware_status::FirmwareStatus;
