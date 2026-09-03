use std::sync::Arc;

use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::semantic::{
    DeviceEpoch, IrqLine, IrqVector, ResourceConfig, WholeDeviceReset,
};

use crate::dma::PdmaMemoryMappings;
use crate::execution::ExecutionUnit;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::queue::exact::PdmaControlError;
use crate::register_file::RegisterFile;
use crate::retirement::RetirementUnit;
use crate::scheduler::Scheduler;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};

#[derive(Debug)]
/// Synchronous ownership root for PDMA hardware state consumed by the async runtime.
pub struct DeviceCore {
    epoch: DeviceEpoch,
    pub(crate) dma: PdmaMemoryMappings,
    pub(crate) registers: RegisterFile,
    queues: QueueBlock,
    scheduler: Scheduler,
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
            dma: PdmaMemoryMappings::new(memory),
            registers: RegisterFile::new(),
            queues: QueueBlock::default(),
            scheduler: Scheduler::default(),
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

    pub(crate) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
        self.scheduler.reset();
        self.retirement.reset();
        self.irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    }

    pub(crate) fn reset_runner(
        &mut self,
        request: WholeDeviceReset,
    ) -> Result<DeviceEpoch, PdmaControlError> {
        let epoch = self.epoch();
        if request.expected_epoch() != epoch {
            return Err(PdmaControlError::StaleEpoch {
                expected: epoch,
                actual: request.expected_epoch(),
            });
        }
        let next = epoch.next()?;
        self.reset(next);
        self.registers = RegisterFile::new();
        self.reset_runner_calls = self.reset_runner_calls.saturating_add(1);
        Ok(next)
    }

    /// Returns committed invocations of the sole reset transition.
    pub const fn reset_runner_calls(&self) -> usize {
        self.reset_runner_calls
    }
}
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "standalone pipeline remains additive until Task 24"
    )
)]
mod command_pipeline;
mod irq;
#[doc(hidden)]
pub(crate) mod pipeline;
mod probe;
mod queue;
mod trace;

pub(crate) use pipeline::ModelStepStatus;

#[cfg(test)]
mod tests;
