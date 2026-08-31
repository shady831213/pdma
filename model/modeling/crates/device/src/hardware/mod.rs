#[doc(hidden)]
pub mod execution;
#[doc(hidden)]
pub mod queue;
#[doc(hidden)]
pub mod retirement;
#[doc(hidden)]
pub mod scheduler;

use std::sync::Arc;

use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::semantic::ResourceConfig;

use crate::ModelResult;
use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::mmio::RegisterBanks;
use crate::abi::ring::control::QueueControlState;
use crate::engine::DeviceExecution;
use crate::engine::step::ModelStepStatus;

#[derive(Debug)]
pub(crate) struct PdmaDevice {
    pub(crate) execution: DeviceExecution,
    pub(crate) dma: PdmaMemoryMappings,
    pub(crate) queue_control: QueueControlState,
    pub(crate) registers: RegisterBanks,
}

impl PdmaDevice {
    pub(crate) fn new(memory: Arc<SharedMemory>, config: ResourceConfig) -> Self {
        Self {
            execution: DeviceExecution::with_resource_config(config),
            dma: PdmaMemoryMappings::new(memory),
            queue_control: QueueControlState::new(),
            registers: RegisterBanks::new(),
        }
    }

    pub(crate) fn step(&mut self) -> ModelResult<ModelStepStatus> {
        self.execution.step(&mut self.dma, &mut self.queue_control)
    }
}
