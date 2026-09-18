//! Software access to the PDMA management channel.

mod addresses;
mod interrupt;
mod layout;
pub use addresses::ManagementAddresses;
pub use interrupt::{ControlIrq, ControlIsr, InterruptEvent};
pub use layout::ControlQueueLayout;
