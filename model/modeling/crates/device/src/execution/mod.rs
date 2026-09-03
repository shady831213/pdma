//! Command execution and deterministic device stepping.

mod broadcast;
#[doc(hidden)]
pub(crate) mod context;
mod copy;
mod memory;
mod resources;
mod vector_add;

use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, Command, CommandOperation, CopyRequest, StridedCopyRequest,
    VectorAddRequest,
};

pub(crate) use context::{
    CommandRoute, ExecutionFault, ExecutionOutcome, RetirableExecution, SuccessfulExecution,
};

#[derive(Debug)]
pub(crate) struct ExecutionUnit {
    resources: ResourceConfig,
    native_steps: usize,
}

impl ExecutionUnit {
    pub(crate) const fn new(resources: ResourceConfig) -> Self {
        Self {
            resources,
            native_steps: 0,
        }
    }

    pub(crate) const fn resources(&self) -> ResourceConfig {
        self.resources
    }

    pub(crate) const fn record_fetch(&mut self) {
        self.native_steps += 1;
    }

    pub(crate) const fn native_steps(&self) -> usize {
        self.native_steps
    }

    pub(crate) fn can_execute(&self, command: &Command) -> bool {
        resources::ResourceDemand::for_command(command).fits(self.resources)
    }

    pub(crate) const fn can_execute_copy(&self) -> bool {
        resources::ResourceDemand::copy().fits(self.resources)
    }

    pub(crate) const fn can_execute_strided(&self, request: StridedCopyRequest) -> bool {
        resources::ResourceDemand::strided(request).fits(self.resources)
    }

    pub(crate) fn can_execute_broadcast(&self, request: &BroadcastCopyRequest) -> bool {
        resources::ResourceDemand::broadcast(request).fits(self.resources)
    }

    pub(crate) const fn can_execute_vector_add(&self) -> bool {
        resources::ResourceDemand::vector_add().fits(self.resources)
    }

    pub(crate) fn execute_copy(
        dma: &crate::dma::PdmaMemoryMappings,
        route: CommandRoute,
        request: CopyRequest,
    ) -> ExecutionOutcome {
        copy::execute(dma, route, request)
    }

    pub(crate) fn execute_strided(
        dma: &crate::dma::PdmaMemoryMappings,
        route: CommandRoute,
        request: StridedCopyRequest,
    ) -> ExecutionOutcome {
        copy::execute_strided(dma, route, request)
    }

    pub(crate) fn execute_broadcast(
        dma: &crate::dma::PdmaMemoryMappings,
        route: CommandRoute,
        request: &BroadcastCopyRequest,
    ) -> ExecutionOutcome {
        broadcast::execute(dma, route, request)
    }

    pub(crate) fn execute_vector_add(
        dma: &crate::dma::PdmaMemoryMappings,
        route: CommandRoute,
        request: VectorAddRequest,
    ) -> ExecutionOutcome {
        vector_add::execute(dma, route, request)
    }

    pub(crate) fn execute(
        &self,
        dma: &crate::dma::PdmaMemoryMappings,
        route: CommandRoute,
        command: Command,
    ) -> Result<ExecutionOutcome, crate::AsyncStallReason> {
        if !self.can_execute(&command) {
            return Err(crate::AsyncStallReason::ExecutionResourcePressure);
        }
        Ok(match command.into_operation() {
            CommandOperation::Copy(request) => copy::execute(dma, route, request),
            CommandOperation::StridedCopy(request) => copy::execute_strided(dma, route, request),
            CommandOperation::BroadcastCopy(request) => broadcast::execute(dma, route, &request),
            CommandOperation::VectorAdd(request) => vector_add::execute(dma, route, request),
        })
    }
}
