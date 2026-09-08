mod block;
mod direct;
pub(crate) mod exact;
pub(crate) mod io;
pub(crate) mod runnable;

#[cfg(test)]
mod direct_tests;

use exact::ExactQueueIo;
use pdma_model_contract::abi::registers::QueueRegistersValues;

#[derive(Debug, Default)]
pub(crate) struct QueueBlock {
    contexts: Vec<PdmaQueue>,
}

#[derive(Debug, Default)]
struct PdmaQueue {
    registers: QueueRegistersValues,
    io: Option<ExactQueueIo>,
}
