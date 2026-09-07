use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::QueueId;

use super::Scheduler;
use crate::device::hardware_context::HardwareContext;
use crate::{AsyncStallReason, ModelError};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectedQueue(pub(crate) Option<QueueId>);

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueueFeedback(pub(crate) QueueId);

impl Scheduler {
    pub(crate) fn accept_feedback(&mut self, feedback: QueueFeedback) {
        self.enqueue(feedback.0);
    }
}

impl Stage<HardwareContext<'_>> for Scheduler {
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
        _: &mut HardwareContext<'_>,
        active: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        if std::mem::replace(active, false) {
            StepOutcome::Complete(Ok(SelectedQueue(self.next())))
        } else {
            StepOutcome::Exhausted
        }
    }
}
