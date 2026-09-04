mod claim;
mod dispatch;
mod global;
pub(crate) mod irq;
mod preparation;
pub(crate) mod queue;
pub(crate) mod reset;

pub(crate) use claim::{claim_observation_count, prepare_claim};
pub use preparation::MmioRegisterError;
pub(crate) use preparation::{
    PreparedRead, PreparedWrite, prepare_bootstrap_batch_write, prepare_read,
    prepare_submission_batch_write, prepare_write,
};
pub(crate) fn q_command_offset() -> u32 {
    pdma_model_contract::abi::mmio::PdmaMmioMap::register(
        pdma_model_contract::abi::mmio::RegisterId::QCommand,
    )
    .and_then(|register| u32::try_from(register.offset()).ok())
    .unwrap_or(u32::MAX)
}

#[cfg(test)]
use pdma_model_contract::abi::mmio::{QueueResponseBank, RegisterId};

#[cfg(test)]
mod tests;
