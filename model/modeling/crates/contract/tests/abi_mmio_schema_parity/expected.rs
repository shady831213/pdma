//! Task-9-independent literal register rows.

mod global;
mod irq;
mod queue;

use pdma_model_contract::abi::mmio::{MmioAccess, MmioByteCount, MmioReset, RegisterId};

/// One independently captured legacy register row.
#[derive(Clone, Copy, Debug)]
pub struct ExpectedRegister {
    /// Legacy identity.
    pub id: RegisterId,
    /// Canonical name.
    pub name: &'static str,
    /// Byte offset.
    pub offset: u32,
    /// Transaction width.
    pub width: MmioByteCount,
    /// Access permission.
    pub access: MmioAccess,
    /// Reset classification.
    pub reset: MmioReset,
    /// Reset value.
    pub reset_value: u64,
    /// Writable mask.
    pub writable_mask: u64,
}

/// Returns all literal rows in canonical order.
pub fn rows() -> impl Iterator<Item = ExpectedRegister> {
    global::REGISTERS
        .into_iter()
        .chain(queue::REGISTERS)
        .chain(irq::REGISTERS)
}
