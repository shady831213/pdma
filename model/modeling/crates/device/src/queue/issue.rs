use hardware_interface_pipeline::{Stage, StepOutcome, Then, ThenState};

use super::QueueBlock;
use super::stage::FetchedWork;
use crate::device::hardware_context::HardwareContext;
use crate::scheduler::Scheduler;
use crate::scheduler::stage::SelectedQueue;
use crate::{AsyncStallReason, ModelError};

pub(crate) struct IssueView<'a> {
    pub(crate) scheduler: &'a mut Scheduler,
    pub(crate) queues: &'a mut QueueBlock,
}

impl Stage<HardwareContext<'_>> for IssueView<'_> {
    type Input = ();
    type Output = FetchedWork;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = ThenState<bool, Option<SelectedQueue>>;

    fn start((): Self::Input) -> Self::State {
        <Then<Scheduler, QueueBlock> as Stage<HardwareContext<'_>>>::start(())
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let mut stages = Then::new(&mut *self.scheduler, &mut *self.queues);
        match stages.step(context, state) {
            StepOutcome::Progressed => stages.step(context, state),
            outcome => outcome,
        }
    }
}
