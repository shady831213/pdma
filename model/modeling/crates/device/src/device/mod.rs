use std::sync::Arc;

use hardware_interface_runtime::{
    ObservationGate, RegisterObserver, SharedMemory, SharedRegisterObserver,
};
use pdma_model_contract::semantic::{
    DeviceEpoch, IrqLine, IrqVector, ResourceConfig, WholeDeviceReset,
};

use crate::AsyncStepStatus;
use crate::dma::PdmaMemoryMappings;
use crate::execution::ExecutionUnit;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::register_file::RegisterFile;
use crate::retirement::RetirementUnit;
use crate::scheduler::Scheduler;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};

#[derive(Debug)]
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
    pub(crate) steps: usize,
    pub(crate) errors: usize,
    pub(crate) stalls: usize,
    pub(crate) last_status: AsyncStepStatus,
    pub(crate) generation: usize,
    pub(crate) wake_generation: usize,
    pub(crate) shutdown_requested: bool,
    pub(crate) register_gate: ObservationGate,
    reset_runner_calls: usize,
    #[cfg(test)]
    pub(crate) batch_fault: std::cell::Cell<Option<crate::abi::ring::BatchPreparationFault>>,
}

impl DeviceCore {
    /// Creates one synchronous device core over system-owned integration capabilities.
    pub fn new(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
    ) -> Self {
        let observer = SharedRegisterObserver::new(observer);
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
            steps: 0,
            errors: 0,
            stalls: 0,
            last_status: AsyncStepStatus::Idle,
            generation: 0,
            wake_generation: 0,
            shutdown_requested: false,
            register_gate: ObservationGate::new(observer),
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

    pub(crate) const fn mark_changed(&mut self) {
        self.wake_generation = self.wake_generation.saturating_add(1);
        self.generation = self.generation.saturating_add(1);
    }

    pub(crate) fn reset_partition_counts(&self) -> (usize, usize, usize, usize, usize) {
        let result_queues = self.queues.result_queue_count();
        (
            self.queues.len(),
            result_queues,
            self.scheduler.len(),
            usize::from(self.retirement.is_pending()),
            self.irq.source_count(),
        )
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
    ) -> Result<DeviceEpoch, crate::abi::ring::PdmaControlError> {
        let epoch = self.epoch();
        if request.expected_epoch() != epoch {
            return Err(crate::abi::ring::PdmaControlError::StaleEpoch {
                expected: epoch,
                actual: request.expected_epoch(),
            });
        }
        let next = epoch.next()?;
        self.reset(next);
        crate::abi::mmio::reset::register_banks(self);
        self.reset_runner_calls = self.reset_runner_calls.saturating_add(1);
        Ok(next)
    }

    /// Returns committed invocations of the sole reset transition.
    pub const fn reset_runner_calls(&self) -> usize {
        self.reset_runner_calls
    }
}
mod irq;
#[doc(hidden)]
pub mod pipeline;
mod probe;
mod queue;
mod trace;

pub(crate) use pipeline::ModelStepStatus;
