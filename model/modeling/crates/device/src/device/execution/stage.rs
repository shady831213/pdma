use crate::operation::DecodedOperation;
use hardware_interface_pipeline::{Stage, StepOutcome, Then, ThenState};
use pdma_model_contract::semantic::ResourceConfig;

use super::ExecutionUnit;
use super::resources::ResourceDemand;
use crate::device::context::HardwareContext;
use crate::device::flow::{ExecutionOutcome, FetchedWork, RetirementWork};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) struct ResourceGate {
    pub(super) config: ResourceConfig,
}

#[derive(Debug)]
pub(super) struct Datapath;

#[derive(Debug)]
pub(in crate::device) struct AdmittedWork(FetchedWork);

impl Stage<HardwareContext<'_>> for ResourceGate {
    type Input = FetchedWork;
    type Output = AdmittedWork;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = Option<FetchedWork>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match state {
            Some(FetchedWork::Command(command)) => {
                if !ResourceDemand::for_operation(&command.operation).fits(self.config) {
                    context
                        .trace
                        .stall(Some(command.route.queue), "ExecutionResourcePressure");
                    return StepOutcome::Pending(AsyncStallReason::ExecutionResourcePressure);
                }
            }
            Some(FetchedWork::Empty(_) | FetchedWork::FrontendFault(_)) | None => {}
        }
        state.take().map_or(StepOutcome::Exhausted, |work| {
            StepOutcome::Complete(Ok(AdmittedWork(work)))
        })
    }
}

impl Stage<HardwareContext<'_>> for Datapath {
    type Input = AdmittedWork;
    type Output = RetirementWork;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = Option<AdmittedWork>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(AdmittedWork(work)) = state.take() else {
            return StepOutcome::Exhausted;
        };
        let result = match work {
            FetchedWork::Empty(status) => RetirementWork::Empty(status),
            FetchedWork::FrontendFault(fault) => RetirementWork::FrontendFault(fault),
            FetchedWork::Command(command) => {
                let route = command.route;
                let outcome: ExecutionOutcome = match command.operation {
                    DecodedOperation::Copy(request) => {
                        ExecutionUnit::execute_copy(context.memory, route, request)
                    }
                    DecodedOperation::StridedCopy(request) => {
                        ExecutionUnit::execute_strided(context.memory, route, request)
                    }
                    DecodedOperation::BroadcastCopy(request) => {
                        ExecutionUnit::execute_broadcast(context.memory, route, &request)
                    }
                    DecodedOperation::VectorAdd(request) => {
                        ExecutionUnit::execute_vector_add(context.memory, route, request)
                    }
                };
                RetirementWork::Executed(outcome)
            }
        };
        StepOutcome::Complete(Ok(result))
    }
}

impl Stage<HardwareContext<'_>> for ExecutionUnit {
    type Input = FetchedWork;
    type Output = RetirementWork;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = ThenState<Option<FetchedWork>, Option<AdmittedWork>>;

    fn start(input: Self::Input) -> Self::State {
        <Then<ResourceGate, Datapath> as Stage<HardwareContext<'_>>>::start(input)
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match self.stages.step(context, state) {
            StepOutcome::Progressed => self.stages.step(context, state),
            outcome => outcome,
        }
    }
}
