use super::port::Frame;
use crate::{ChannelTask, Dma, Error, Fetch, RunCommand};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};

#[derive(Debug)]
pub(super) struct Engine<S>(pub RunCommand<S>);

#[derive(Debug)]
pub(super) enum EngineState<T> {
    Selected(ChannelId),
    Running {
        channel: ChannelId,
        child: Option<T>,
    },
    Finished,
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Frame<'_, S::State, M, SB>> for Engine<S>
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error>,
{
    type Input = ChannelId;
    type Output = ();
    type Error = Error;
    type State = EngineState<S::State>;

    fn start(channel: ChannelId) -> Self::State {
        EngineState::Selected(channel)
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, S::State, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<(), Error> {
        match state {
            EngineState::Selected(channel) => {
                let channel = *channel;
                if !Fetch::new(channel).eligible(frame.device) {
                    *state = EngineState::Finished;
                    return Ok(StepOutcome::Complete(Ok(())));
                }
                let Some(slot) = frame.saved.get_mut(channel.index()) else {
                    *state = EngineState::Finished;
                    return Ok(StepOutcome::Complete(Err(Error::Routing)));
                };
                let child = if let Some(child) = slot.take() {
                    frame.device.active = frame
                        .device
                        .channels
                        .get_mut(channel.index())
                        .and_then(|channel| channel.parked.take());
                    child
                } else {
                    match Fetch::new(channel).step(frame.device, &mut ())? {
                        StepOutcome::Complete(Ok(task)) => S::start(task),
                        StepOutcome::Complete(Err(error)) => {
                            *state = EngineState::Finished;
                            return Ok(StepOutcome::Complete(match error {
                                Error::Reset | Error::ChannelFatal(_) | Error::Halted => Ok(()),
                                error => Err(error),
                            }));
                        }
                        StepOutcome::InProgress => {
                            *state = EngineState::Finished;
                            return Ok(StepOutcome::Complete(Ok(())));
                        }
                    }
                };
                *state = EngineState::Running {
                    channel,
                    child: Some(child),
                };
                Ok(StepOutcome::InProgress)
            }
            EngineState::Running { channel, child } => match self.0.step(frame.device, child)? {
                StepOutcome::Complete(result) => {
                    *state = EngineState::Finished;
                    Ok(StepOutcome::Complete(result.map(|_| ())))
                }
                StepOutcome::InProgress => {
                    let Some(owner) = frame.device.channels.get_mut(channel.index()) else {
                        return Ok(StepOutcome::Complete(Err(Error::Routing)));
                    };
                    if owner.registers.values.fault_status.faulted() {
                        let Some(slot) = frame.saved.get_mut(channel.index()) else {
                            return Ok(StepOutcome::Complete(Err(Error::Routing)));
                        };
                        *slot = child.take();
                        owner.parked = frame.device.active.take();
                        *state = EngineState::Finished;
                        Ok(StepOutcome::Complete(Ok(())))
                    } else {
                        Ok(StepOutcome::InProgress)
                    }
                }
            },
            EngineState::Finished => Err(RunError::Exhausted),
        }
    }
}
