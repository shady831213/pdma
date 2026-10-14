use crate::channel::{Accepted, Action, RangeCheck};
use crate::{CommandRequest, Dma, Error, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

/// Accepts an eligible snapshot into channel-owned descriptor state.
#[derive(Debug, Default)]
pub struct Fetch;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Fetch {
    type Input = Option<CommandRequest>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Self::Input>;

    fn start(request: Self::Input) -> Self::State {
        Some(request)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(request) = state.take().ok_or(RunError::Exhausted)? else {
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        let mut work = match request {
            CommandRequest::Fetch(work)
            | CommandRequest::Progress(work)
            | CommandRequest::Resume(work) => work,
        };
        match device.command_request(work.channel) {
            Ok(Some(CommandRequest::Fetch(current)))
                if matches!(request, CommandRequest::Fetch(_)) =>
            {
                work = current;
            }
            Ok(Some(current)) if current == request => {}
            Ok(_) => return Ok(StepOutcome::Complete(Ok(None))),
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
        }
        let selected = match device.channel_context(work.channel) {
            Ok(selected) => selected,
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
        };
        if selected.channel.work.is_none() {
            match selected.channel.command.pop(&selected.memory) {
                Ok(Some(command)) => {
                    selected.channel.work = Some(Accepted {
                        command,
                        action: Action::Check(RangeCheck::new(command)),
                    });
                }
                Ok(None) => return Ok(StepOutcome::Complete(Ok(None))),
                Err(fatal) => {
                    selected.channel.latch(fatal);
                    return Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))));
                }
            }
        }
        device.active = Some(work);
        Ok(StepOutcome::Complete(Ok(Some(work))))
    }
}
