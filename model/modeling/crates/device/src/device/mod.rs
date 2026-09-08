use std::sync::Arc;

use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector, ResourceConfig};

use crate::device::execution::ExecutionUnit;
use crate::device::retirement::RetirementUnit;
use crate::irq::PdmaIrqController;
use crate::memory::PdmaMemoryMappings;
use crate::queue::QueueBlock;
use crate::queue::runnable::RunnableQueues;
use crate::registers::PdmaRegisters;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};

#[derive(Debug)]
/// Synchronous ownership root for PDMA hardware state consumed by the async runtime.
///
/// Its reusable `Stage<()>` interface services one singleton device request at a time.
/// Accepted work belongs to the device across driving-future cancellation. Resetting a
/// generic operation does not reset that hardware; whole-device reset invalidates it.
pub struct DeviceCore {
    epoch: DeviceEpoch,
    command_pipeline: command_pipeline::PipelineSlot,
    native_fetches: usize,
    pub(crate) dma: PdmaMemoryMappings,
    pub(crate) registers: PdmaRegisters,
    queues: QueueBlock,
    runnable: RunnableQueues,
    issue: issue::IssueUnit,
    execution: ExecutionUnit,
    retirement: RetirementUnit,
    irq: PdmaIrqController,
    trace: TraceSink,
    reset_runner_calls: usize,
    #[cfg(test)]
    pub(crate) batch_fault: std::cell::Cell<Option<crate::interface::batch::BatchPreparationFault>>,
}

impl DeviceCore {
    /// Creates one synchronous device core over system-owned integration capabilities.
    pub fn new(memory: Arc<SharedMemory>, config: ResourceConfig) -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            command_pipeline: command_pipeline::PipelineSlot::new(),
            native_fetches: 0,
            dma: PdmaMemoryMappings::new(memory),
            registers: PdmaRegisters::new(),
            queues: QueueBlock::default(),
            runnable: RunnableQueues::default(),
            issue: issue::IssueUnit::default(),
            execution: ExecutionUnit::new(config),
            retirement: RetirementUnit::default(),
            irq: PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0))),
            trace: TraceSink::default(),
            reset_runner_calls: 0,
            #[cfg(test)]
            batch_fault: std::cell::Cell::new(None),
        }
    }

    pub(super) const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    pub(crate) fn validate_epoch(&self, actual: DeviceEpoch) -> ModelResult<()> {
        if actual == self.epoch {
            Ok(())
        } else {
            Err(ModelError::StaleDeviceEpoch {
                expected: self.epoch,
                actual,
            })
        }
    }

    /// Returns committed invocations of the sole reset transition.
    pub const fn reset_runner_calls(&self) -> usize {
        self.reset_runner_calls
    }
}
mod command_pipeline;
mod context;
mod control;
#[doc(hidden)]
pub(crate) mod drive;
mod execution;
mod flow;
mod issue;
mod probe;
mod retirement;
mod trace;

pub(crate) use drive::ModelStepStatus;

#[cfg(test)]
mod tests;
