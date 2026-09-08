mod claim;
mod global;
pub(crate) mod irq;
mod preparation;
pub(crate) mod queue;
pub(crate) mod reset;

pub(crate) use claim::{claim_observation_count, prepare_claim};
pub use preparation::MmioRegisterError;
pub(crate) use preparation::{PreparedRead, PreparedWrite, prepare_read, prepare_write};

#[cfg(test)]
use pdma_model_contract::abi::mmio::{QueueResponseBank, RegisterId};

#[cfg(test)]
mod tests;
