//! Command execution and deterministic device stepping.

mod broadcast;
#[doc(hidden)]
pub(crate) mod context;
mod copy;
mod memory;
mod resources;
pub(crate) mod stage;
mod vector_add;

use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, Command, CopyRequest, StridedCopyRequest, VectorAddRequest,
};

pub(crate) use context::{
    CommandRoute, ExecutionFault, ExecutionOutcome, RetirableExecution, SuccessfulExecution,
};

#[derive(Debug)]
pub(crate) struct ExecutionUnit {
    stages: hardware_interface_pipeline::Then<stage::ResourceGate, stage::Datapath>,
}

impl ExecutionUnit {
    pub(crate) const fn new(resources: ResourceConfig) -> Self {
        Self {
            stages: hardware_interface_pipeline::Then::new(
                stage::ResourceGate { config: resources },
                stage::Datapath,
            ),
        }
    }

    pub(crate) const fn resources(&self) -> ResourceConfig {
        self.stages.upstream.config
    }

    pub(crate) fn can_execute(&self, command: &Command) -> bool {
        resources::ResourceDemand::for_command(command).fits(self.resources())
    }

    pub(crate) fn execute_copy(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: CopyRequest,
    ) -> ExecutionOutcome {
        copy::execute(dma, route, request)
    }

    pub(crate) fn execute_strided(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: StridedCopyRequest,
    ) -> ExecutionOutcome {
        copy::execute_strided(dma, route, request)
    }

    pub(crate) fn execute_broadcast(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: &BroadcastCopyRequest,
    ) -> ExecutionOutcome {
        broadcast::execute(dma, route, request)
    }

    pub(crate) fn execute_vector_add(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: VectorAddRequest,
    ) -> ExecutionOutcome {
        vector_add::execute(dma, route, request)
    }
}
