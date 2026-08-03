//! Independent literal register expectations split only for source-size limits.

mod global;
mod irq;
mod queue;

use pdma_model_contract::abi::mmio::{MmioAccess, MmioReset, RegisterId};

/// One literal register row independent of the production registry.
pub type RegisterLiteral = (
    RegisterId,
    &'static str,
    u32,
    u128,
    MmioAccess,
    MmioReset,
    u64,
    u64,
);

/// Returns the literal register count.
pub const fn len() -> usize {
    66
}

/// Iterates literal rows in ascending offset order.
pub fn iter() -> impl Iterator<Item = RegisterLiteral> {
    global::REGISTERS
        .into_iter()
        .chain(queue::REGISTERS)
        .chain(irq::REGISTERS)
}
