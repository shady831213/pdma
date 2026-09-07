use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::CommandOperation;

use super::arbitrate::SelectedQueue;
use crate::device::ModelStepStatus;
use crate::device::hardware_context::HardwareContext;
use crate::execution::CommandRoute;
use crate::queue::io::{NormalizedFault, NormalizedFetch};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(crate) struct FetchedCommand {
    pub(crate) route: CommandRoute,
    pub(crate) operation: CommandOperation,
}

#[derive(Debug)]
pub(crate) enum FetchedWork {
    Empty(ModelStepStatus),
    FrontendFault(NormalizedFault),
    Command(FetchedCommand),
}

#[derive(Debug)]
pub(super) struct Fetch;

impl Stage<HardwareContext<'_>> for Fetch {
    type Input = SelectedQueue;
    type Output = FetchedWork;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = Option<SelectedQueue>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(selected) = state.take() else {
            return StepOutcome::Exhausted;
        };
        StepOutcome::Complete(fetch_selected(context, selected))
    }
}

fn fetch_selected(
    context: &mut HardwareContext<'_>,
    selected: SelectedQueue,
) -> Result<FetchedWork, ModelError> {
    let Some(queue) = selected.0 else {
        return Ok(FetchedWork::Empty(ModelStepStatus::Idle));
    };
    let Some(fetched) = context.queues.fetch(context.dma, queue)? else {
        return Ok(FetchedWork::Empty(ModelStepStatus::Progressed));
    };
    *context.fetches += 1;
    Ok(match fetched {
        NormalizedFetch::Fault(fault) => FetchedWork::FrontendFault(fault),
        NormalizedFetch::Command {
            command,
            completion_ring,
        } => {
            let route = CommandRoute::new(
                queue,
                command.command_id(),
                completion_ring,
                context.queues.contains_completion(completion_ring),
            );
            context.trace.scheduled(
                queue,
                command.operation().trace_operation(),
                command.command_id(),
            );
            FetchedWork::Command(FetchedCommand {
                route,
                operation: command.into_operation(),
            })
        }
    })
}
