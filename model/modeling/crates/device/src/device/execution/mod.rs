//! Command execution and deterministic device stepping.

#[cfg(test)]
mod backend_tests;
mod broadcast;
mod copy;
mod memory;
pub(super) mod resources;
pub(super) mod stage;
mod vector_add;

use crate::operation::{
    BroadcastOperands, CopyOperands, DecodedCommand, StridedOperands, VectorOperands,
};
use pdma_model_contract::semantic::ResourceConfig;

use super::flow::{CommandRoute, ExecutionOutcome, SuccessfulExecution};

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

    pub(super) fn can_execute(&self, command: &DecodedCommand) -> bool {
        resources::ResourceDemand::for_command(command).fits(self.resources())
    }

    pub(super) fn execute_copy(
        memory: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: CopyOperands,
    ) -> ExecutionOutcome {
        copy::execute(memory, route, request)
    }

    pub(super) fn execute_strided(
        memory: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: StridedOperands,
    ) -> ExecutionOutcome {
        copy::execute_strided(memory, route, request)
    }

    pub(super) fn execute_broadcast(
        memory: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: &BroadcastOperands,
    ) -> ExecutionOutcome {
        broadcast::execute(memory, route, request)
    }

    pub(super) fn execute_vector_add(
        memory: &crate::memory::PdmaMemoryMappings,
        route: CommandRoute,
        request: VectorOperands,
    ) -> ExecutionOutcome {
        vector_add::execute(memory, route, request)
    }
}
