use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::QueueId;

use crate::device::context::HardwareContext;
use crate::device::execution::resources::ResourceDemand;
use crate::queue::io::NormalizedFetch;
use crate::{AsyncStallReason, ModelError};

#[derive(Debug, Clone, Copy)]
pub(in crate::device) struct SelectedQueue(pub(in crate::device) Option<QueueId>);

#[derive(Debug)]
pub(super) struct Arbitration;

impl Stage<HardwareContext<'_>> for Arbitration {
    type Input = ();
    type Output = SelectedQueue;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = bool;

    fn start((): Self::Input) -> Self::State {
        true
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        active: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        if !*active {
            return StepOutcome::Exhausted;
        }
        let mut first_starved = None;
        for _ in 0..context.runnable.runnable_len() {
            let Some(queue) = context.runnable.current() else {
                break;
            };
            match context.queues.probe(context.memory, queue) {
                Ok(Some(NormalizedFetch::Command { command, .. }))
                    if !ResourceDemand::for_command(&command).fits(context.resources) =>
                {
                    first_starved.get_or_insert(queue);
                    context.runnable.rotate_current();
                }
                Ok(Some(NormalizedFetch::Command { .. } | NormalizedFetch::Fault(_)) | None)
                | Err(_) => {
                    *active = false;
                    return StepOutcome::Complete(Ok(SelectedQueue(context.runnable.next())));
                }
            }
        }
        if let Some(queue) = first_starved {
            context
                .trace
                .stall(Some(queue), "ExecutionResourcePressure");
            return StepOutcome::Pending(AsyncStallReason::ExecutionResourcePressure);
        }
        *active = false;
        StepOutcome::Complete(Ok(SelectedQueue(None)))
    }
}
