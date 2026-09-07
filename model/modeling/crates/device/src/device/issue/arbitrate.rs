use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::QueueId;

use crate::device::hardware_context::HardwareContext;
use crate::{AsyncStallReason, ModelError};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedQueue(pub(crate) Option<QueueId>);

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
        if std::mem::replace(active, false) {
            StepOutcome::Complete(Ok(SelectedQueue(context.scheduler.next())))
        } else {
            StepOutcome::Exhausted
        }
    }
}
