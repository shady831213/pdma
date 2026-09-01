//! Command execution and deterministic device stepping.

mod broadcast;
#[doc(hidden)]
pub mod context;
mod copy;
mod memory;
mod resources;
mod vector_add;

use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_contract::semantic::{Command, CommandOperation};

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
