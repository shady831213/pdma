use crate::channel::{Action, COPY_CHUNK_BYTES, Transfer};
use crate::{Dma, Error, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

/// Selects the next bounded chunk using the channel-owned traversal cursors.
#[derive(Debug, Default)]
pub struct NextChunk;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for NextChunk {
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
        let mut complete = true;
        accepted.action = match accepted.action {
            Action::Next(mut progress) => {
                if let Some((source, destination, length)) =
                    progress.prepare(&context.memory_for(progress.command.sg_context))
                {
                    Action::Read(Transfer {
                        progress,
                        source,
                        destination,
                        length,
                        bytes: [0; COPY_CHUNK_BYTES],
                    })
                } else {
                    complete = false;
                    Action::Next(progress)
                }
            }
            action @ (Action::Check(_)
            | Action::Read(_)
            | Action::Transform(_)
            | Action::Write(_)
            | Action::Respond(_)) => action,
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
