//! Command execution and deterministic device stepping.

mod broadcast;
mod copy;
mod memory;
pub(super) mod resources;
pub(super) mod stage;
mod vector_add;

use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, Command, CopyRequest, StridedCopyRequest, VectorAddRequest,
};

use super::flow::{CommandRoute, ExecutionFault, ExecutionOutcome, SuccessfulExecution};

const fn success(
    route: CommandRoute,
    operation: pdma_model_contract::semantic::TraceOperation,
) -> ExecutionOutcome {
    ExecutionOutcome::Success(SuccessfulExecution { route, operation })
}

#[derive(Debug)]
pub(super) struct ExecutionUnit {
    stages: hardware_interface_pipeline::Then<stage::ResourceGate, stage::Datapath>,
}

impl ExecutionUnit {
    pub(super) const fn new(resources: ResourceConfig) -> Self {
        Self {
            stages: hardware_interface_pipeline::Then::new(
                stage::ResourceGate { config: resources },
                stage::Datapath,
            ),
        }
    }

    pub(super) const fn resources(&self) -> ResourceConfig {
        self.stages.upstream.config
    }

    pub(super) fn can_execute(&self, command: &Command) -> bool {
        resources::ResourceDemand::for_command(command).fits(self.resources())
    }

    pub(super) fn execute_copy(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: CopyRequest,
    ) -> ExecutionOutcome {
        copy::execute(dma, route, request)
    }

    pub(super) fn execute_strided(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: StridedCopyRequest,
    ) -> ExecutionOutcome {
        copy::execute_strided(dma, route, request)
    }

    pub(super) fn execute_broadcast(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: &BroadcastCopyRequest,
    ) -> ExecutionOutcome {
        broadcast::execute(dma, route, request)
    }

    pub(super) fn execute_vector_add(
        dma: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: VectorAddRequest,
    ) -> ExecutionOutcome {
        vector_add::execute(dma, route, request)
    }
}
