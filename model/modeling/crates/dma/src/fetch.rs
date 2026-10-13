use crate::{ChannelTask, CommandRequest, Dma, Error};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};

/// Accepts one command from the channel supplied through its input.
#[derive(Debug, Default)]
pub struct Fetch;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Fetch {
    type Input = ChannelId;
    type Output = ChannelTask<Command>;
    type Error = Error;
    type State = Option<ChannelId>;

    fn start(channel: ChannelId) -> Self::State {
        Some(channel)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let channel = state.take().ok_or(RunError::Exhausted)?;
        match device.command_request(channel) {
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            Ok(Some(CommandRequest::Fetch(_))) => {}
            Ok(None | Some(CommandRequest::Resume(_))) => {
                *state = Some(channel);
                return Ok(StepOutcome::InProgress);
            }
        }
        let outcome = {
            let selected = match device.channel_context(channel) {
                Ok(selected) => selected,
                Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            };
            let outcome = match selected.channel.command.pop(&selected.memory) {
                Ok(Some(value)) => StepOutcome::Complete(Ok(ChannelTask {
                    channel,
                    generation: selected.channel.generation,
                    value,
                })),
                Ok(None) => {
                    *state = Some(channel);
                    StepOutcome::InProgress
                }
                Err(fatal) => {
                    selected.channel.latch(fatal);
                    StepOutcome::Complete(Err(Error::ChannelFatal(fatal)))
                }
            };
            selected.channel.update_irq();
            outcome
        };
        if let StepOutcome::Complete(Ok(task)) = &outcome {
            device.active = Some(ChannelTask {
                channel: task.channel(),
                generation: task.generation,
                value: crate::references(&task.value),
            });
        }
        Ok(outcome)
    }
}
