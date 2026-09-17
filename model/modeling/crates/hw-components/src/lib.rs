//! Concrete management access-window hardware.

mod access_window;
mod channel_windows;
mod control_channel;
mod error;
mod firmware_status;

pub use access_window::AccessWindow;
pub use channel_windows::ChannelWindows;
pub use control_channel::{ControlChannel, ControlError};
pub use error::{AccessError, WindowConfigError};
pub use firmware_status::FirmwareStatus;
