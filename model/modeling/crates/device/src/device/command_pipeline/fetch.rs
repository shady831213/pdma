use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, CommandOperation, CopyRequest, StridedCopyRequest, VectorAddRequest,
};

use super::super::probe::{RunStateProbe, probe_run_state};
use super::super::{DeviceCore, ModelStepStatus};
use crate::execution::CommandRoute;
use crate::queue::io::{NormalizedFault, NormalizedFetch};
use crate::retirement::PendingRetirement;
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) struct ProbeFetch {
    state: State,
}

#[derive(Debug)]
enum State {
    Probe,
    Fetch,
    Done,
}

#[derive(Debug)]
pub(super) enum FetchedRoute {
    Terminal(ModelStepStatus),
    RetryCompletion,
    RetryFault,
    FrontendFault(NormalizedFault),
    Copy(CommandRoute, CopyRequest),
    StridedCopy(CommandRoute, StridedCopyRequest),
    BroadcastCopy(CommandRoute, BroadcastCopyRequest),
    VectorAdd(CommandRoute, VectorAddRequest),
}

impl ProbeFetch {
    pub(super) const fn new() -> Self {
        Self {
            state: State::Probe,
        }
    }

    fn fetch(context: &mut DeviceCore) -> Result<FetchedRoute, ModelError> {
        if let Some(pending) = context.retirement.pending() {
            return Ok(match pending {
                PendingRetirement::Completion(_) => FetchedRoute::RetryCompletion,
                PendingRetirement::Fault { .. } => FetchedRoute::RetryFault,
            });
        }
        let Some(queue) = context.scheduler.next() else {
            return Ok(FetchedRoute::Terminal(ModelStepStatus::Idle));
        };
        let Some(fetched) = context.queues.fetch(&context.dma, queue)? else {
            return Ok(FetchedRoute::Terminal(ModelStepStatus::Progressed));
        };
        context.execution.record_fetch();
        match fetched {
            NormalizedFetch::Fault(fault) => Ok(FetchedRoute::FrontendFault(fault)),
            NormalizedFetch::Command {
                command,
                completion_ring,
            } => {
                let operation = command.operation().trace_operation();
                let route = CommandRoute::new(
                    queue,
                    command.command_id(),
                    completion_ring,
                    context.queues.contains_completion(completion_ring),
                );
                context
                    .trace
                    .scheduled(queue, operation, command.command_id());
                Ok(match command.into_operation() {
                    CommandOperation::Copy(request) => FetchedRoute::Copy(route, request),
                    CommandOperation::StridedCopy(request) => {
                        FetchedRoute::StridedCopy(route, request)
                    }
                    CommandOperation::BroadcastCopy(request) => {
                        FetchedRoute::BroadcastCopy(route, request)
                    }
                    CommandOperation::VectorAdd(request) => FetchedRoute::VectorAdd(route, request),
                })
            }
        }
    }
}

impl Stage<DeviceCore> for ProbeFetch {
    type Error = ModelError;
    type Output = FetchedRoute;
    type Wait = AsyncStallReason;

    fn step(
        &mut self,
        context: &mut DeviceCore,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match std::mem::replace(&mut self.state, State::Done) {
            State::Probe => match probe_run_state(
                &context.queues,
                &context.dma,
                &context.scheduler,
                &context.execution,
                &context.retirement,
            ) {
                RunStateProbe::Idle => {
                    context.trace.idle();
                    StepOutcome::Complete(Ok(FetchedRoute::Terminal(ModelStepStatus::Idle)))
                }
                RunStateProbe::Running | RunStateProbe::Blocked { .. } => {
                    self.state = State::Fetch;
                    StepOutcome::Progressed
                }
            },
            State::Fetch => StepOutcome::Complete(Self::fetch(context)),
            State::Done => StepOutcome::Exhausted,
        }
    }
}
