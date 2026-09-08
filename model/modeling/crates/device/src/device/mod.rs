use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector, ResourceConfig};

use crate::device::execution::ExecutionUnit;
use crate::device::retirement::RetirementUnit;
use crate::irq::PdmaIrqController;
use crate::memory::PdmaMemoryMappings;
use crate::queue::QueueBlock;
use crate::queue::runnable::RunnableQueues;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::mmio::GlobalRegistersValues;

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
    pub(crate) memory: PdmaMemoryMappings,
    pub(crate) global_registers: GlobalRegistersValues,
    pub(crate) queues: QueueBlock,
    runnable: RunnableQueues,
    issue: issue::IssueUnit,
    execution: ExecutionUnit,
    retirement: RetirementUnit,
    pub(crate) irq: PdmaIrqController,
    trace: TraceSink,
    reset_runner_calls: usize,
}

impl DeviceCore {
    /// Creates one synchronous device core over system-owned integration capabilities.
    pub fn new(memory: impl Into<SharedMemoryAccess>, config: ResourceConfig) -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            command_pipeline: command_pipeline::PipelineSlot::new(),
            native_fetches: 0,
            memory: PdmaMemoryMappings::new(memory),
            global_registers: GlobalRegistersValues::default(),
            queues: QueueBlock::default(),
            runnable: RunnableQueues::default(),
            issue: issue::IssueUnit::default(),
            execution: ExecutionUnit::new(config),
            retirement: RetirementUnit::default(),
            irq: PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0))),
            trace: TraceSink::default(),
            reset_runner_calls: 0,
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
pub(crate) mod mmio;
mod probe;
mod retirement;
mod trace;

pub(crate) use drive::ModelStepStatus;

#[cfg(test)]
mod tests;
