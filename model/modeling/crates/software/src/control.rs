//! Software access to the PDMA management channel.

mod interrupt;
mod layout;
pub use interrupt::{ControlIrq, ControlIsr, InterruptEvent};
pub use layout::ControlQueueLayout;
