use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, CopyRequest, StridedCopyRequest, VectorAddRequest,
};

use super::super::DeviceCore;
use crate::execution::{CommandRoute, ExecutionFault, ExecutionOutcome, SuccessfulExecution};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) struct Copy {
    pending: Option<(CommandRoute, CopyRequest)>,
}

#[derive(Debug)]
pub(super) struct StridedCopy {
    pending: Option<(CommandRoute, StridedCopyRequest)>,
}

#[derive(Debug)]
pub(super) struct BroadcastCopy {
    pending: Option<(CommandRoute, BroadcastCopyRequest)>,
}

#[derive(Debug)]
pub(super) struct VectorAdd {
    pending: Option<(CommandRoute, VectorAddRequest)>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ExecutedRoute {
    Success(SuccessfulExecution),
    Fault(ExecutionFault),
    InvalidCompletion(CommandRoute),
}

impl Copy {
    pub(super) const fn new(route: CommandRoute, request: CopyRequest) -> Self {
        Self {
            pending: Some((route, request)),
        }
    }
}

impl StridedCopy {
    pub(super) const fn new(route: CommandRoute, request: StridedCopyRequest) -> Self {
        Self {
            pending: Some((route, request)),
        }
    }
}

impl BroadcastCopy {
    pub(super) const fn new(route: CommandRoute, request: BroadcastCopyRequest) -> Self {
        Self {
            pending: Some((route, request)),
        }
    }
}

impl VectorAdd {
    pub(super) const fn new(route: CommandRoute, request: VectorAddRequest) -> Self {
        Self {
            pending: Some((route, request)),
        }
    }
}

impl Stage<DeviceCore> for Copy {
    type Error = ModelError;
    type Output = ExecutedRoute;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> ExecuteOutcome {
        let Some((route, request)) = self.pending.take() else {
            return StepOutcome::Exhausted;
        };
        if !context.execution.can_execute_copy() {
            self.pending = Some((route, request));
            return pending(context, route);
        }
        complete(crate::execution::ExecutionUnit::execute_copy(
            &context.dma,
            route,
            request,
        ))
    }
}

impl Stage<DeviceCore> for StridedCopy {
    type Error = ModelError;
    type Output = ExecutedRoute;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> ExecuteOutcome {
        let Some((route, request)) = self.pending.take() else {
            return StepOutcome::Exhausted;
        };
        if !context.execution.can_execute_strided(request) {
            self.pending = Some((route, request));
            return pending(context, route);
        }
        complete(crate::execution::ExecutionUnit::execute_strided(
            &context.dma,
            route,
            request,
        ))
    }
}

impl Stage<DeviceCore> for BroadcastCopy {
    type Error = ModelError;
    type Output = ExecutedRoute;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> ExecuteOutcome {
        let Some((route, request)) = self.pending.take() else {
            return StepOutcome::Exhausted;
        };
        if !context.execution.can_execute_broadcast(&request) {
            self.pending = Some((route, request));
            return pending(context, route);
        }
        complete(crate::execution::ExecutionUnit::execute_broadcast(
            &context.dma,
            route,
            &request,
        ))
    }
}

impl Stage<DeviceCore> for VectorAdd {
    type Error = ModelError;
    type Output = ExecutedRoute;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> ExecuteOutcome {
        let Some((route, request)) = self.pending.take() else {
            return StepOutcome::Exhausted;
        };
        if !context.execution.can_execute_vector_add() {
            self.pending = Some((route, request));
            return pending(context, route);
        }
        complete(crate::execution::ExecutionUnit::execute_vector_add(
            &context.dma,
            route,
            request,
        ))
    }
}

type ExecuteOutcome = StepOutcome<ExecutedRoute, ModelError, AsyncStallReason>;

const fn complete(outcome: ExecutionOutcome) -> ExecuteOutcome {
    StepOutcome::Complete(Ok(match outcome {
        ExecutionOutcome::Success(success) => ExecutedRoute::Success(success),
        ExecutionOutcome::Fault(fault) => ExecutedRoute::Fault(fault),
        ExecutionOutcome::InvalidCompletion(route) => ExecutedRoute::InvalidCompletion(route),
    }))
}

fn pending(context: &mut DeviceCore, route: CommandRoute) -> ExecuteOutcome {
    match context.queues.has_pending(route.queue) {
        Ok(has_pending) => {
            if has_pending {
                context.scheduler.enqueue(route.queue);
            }
            context
                .trace
                .stall(Some(route.queue), "ExecutionResourcePressure");
            StepOutcome::Pending(AsyncStallReason::ExecutionResourcePressure)
        }
        Err(error) => StepOutcome::Complete(Err(error)),
    }
}
