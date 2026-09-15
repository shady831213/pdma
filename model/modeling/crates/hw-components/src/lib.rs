//! Concrete management access-window hardware.

mod access_window;
mod error;

pub use access_window::AccessWindow;
pub use error::{AccessError, WindowConfigError};
