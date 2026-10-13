use crate::{ChannelTask, CommandInput, CommandRequest, Dma, Error};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::ChannelId;

/// Transfers stored traversal or accepts a fresh descriptor from its input channel.
#[derive(Debug, Default)]
pub struct Fetch;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Fetch {
    type Input = ChannelId;
    type Output = ChannelTask<CommandInput>;
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
        if device
            .active
            .as_ref()
            .is_some_and(|active| active.channel() == channel)
        {
            let owner = device
                .channels
                .get(channel.index())
                .ok_or(RunError::Exhausted)?;
            if device.resetting() || owner.resetting() {
                return Ok(StepOutcome::Complete(Err(Error::Reset)));
            }
            if device.halted() || owner.halted() {
                return Ok(StepOutcome::Complete(Err(Error::Halted)));
            }
        }
        match device.command_request(channel) {
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            Ok(Some(CommandRequest::Fetch(_))) => {}
            Ok(Some(CommandRequest::Progress(_))) => {
                let Some(owner) = device.channels.get_mut(channel.index()) else {
                    return Ok(StepOutcome::Complete(Err(Error::Unmapped)));
                };
                let Some(progress) = owner.progress.take() else {
                    return Ok(StepOutcome::Complete(Err(Error::Routing)));
                };
                return Ok(StepOutcome::Complete(Ok(ChannelTask {
                    channel,
                    generation: owner.generation,
                    value: CommandInput::Progress(progress),
                })));
            }
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
        Ok(match outcome {
            StepOutcome::InProgress => StepOutcome::InProgress,
            StepOutcome::Complete(result) => {
                StepOutcome::Complete(result.map(|task| task.map(CommandInput::Fresh)))
            }
        })
    }
}
