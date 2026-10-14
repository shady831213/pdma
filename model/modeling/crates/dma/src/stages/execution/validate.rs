use crate::channel::Action;
use crate::{Dma, Error, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

/// Incrementally validates both channel-owned endpoint ranges before payload writes.
#[derive(Debug, Default)]
pub struct CheckRanges;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for CheckRanges {
    type Input = Option<Work>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Option<Work>>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(work) = *state.as_ref().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        match device.work_ready(work) {
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
            Ok(false) => return Ok(StepOutcome::InProgress),
            Ok(true) => {}
        }
        let context = match device.channel_context(work.channel()) {
            Ok(context) => context,
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
        };
        let mut accepted = context.channel.work.take().ok_or(RunError::Exhausted)?;
        let complete = match &mut accepted.action {
            Action::Check(scan) => {
                if let Some(progress) = scan.advance(&context.memory_for(scan.command.sg_context)) {
                    accepted.action = Action::Next(progress);
                    true
                } else {
                    false
                }
            }
            Action::Next(_)
            | Action::Read(_)
            | Action::Transform(_)
            | Action::Write(_)
            | Action::Respond(_) => true,
        };
        context.channel.work = Some(accepted);
        if complete {
            *state = None;
            Ok(StepOutcome::Complete(Ok(Some(work))))
        } else {
            Ok(StepOutcome::InProgress)
        }
    }
}
