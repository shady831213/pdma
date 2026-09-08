mod block;
mod direct;
pub(crate) mod exact;
pub(crate) mod io;
mod irq;
pub(crate) mod runnable;

#[cfg(test)]
mod direct_tests;
#[cfg(test)]
mod irq_tests;

use crate::irq::source::IrqSourceState;
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
    completion_irq: IrqSourceState,
    event_irq: IrqSourceState,
}
