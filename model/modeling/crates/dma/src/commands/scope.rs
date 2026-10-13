use super::Frame;
use crate::{CommandRequest, Dma, Error};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::ChannelId;

/// DMA lifecycle combinator: suspends and restores its owned service composition.
#[derive(Debug)]
pub(super) struct CommandScope<S>(pub S);

#[derive(Debug)]
pub(super) enum ScopeState<T> {
    Requested(Option<CommandRequest>),
    Running { channel: ChannelId, child: T },
    Finished,
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Frame<'_, S::State, M, SB>> for CommandScope<S>
where
    S: Stage<Dma<M, SB>, Input = ChannelId, Output = (), Error = Error>,
{
    type Input = Option<CommandRequest>;
    type Output = ();
    type Error = Error;
    type State = ScopeState<S::State>;

    fn start(channel: Self::Input) -> Self::State {
        ScopeState::Requested(channel)
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, S::State, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<(), Error> {
        match std::mem::replace(state, ScopeState::Finished) {
            ScopeState::Requested(request) => {
                let Some(request) = request else {
                    return Ok(StepOutcome::Complete(Ok(())));
                };
                let channel = request.channel();
                let current = match frame.device.command_request(channel) {
                    Ok(current) => current,
                    Err(error) => return Ok(StepOutcome::Complete(Err(error))),
                };
                if current != Some(request) {
                    return Ok(StepOutcome::Complete(Ok(())));
                }
                let child = match request {
                    CommandRequest::Fetch(channel) | CommandRequest::Progress(channel) => {
                        S::start(channel)
                    }
                    CommandRequest::Resume(channel) => {
                        let Some(child) =
                            frame.saved.get_mut(channel.index()).and_then(Option::take)
                        else {
                            return Ok(StepOutcome::Complete(Err(Error::Routing)));
                        };
                        frame.device.active = frame
                            .device
                            .channels
                            .get_mut(channel.index())
                            .and_then(|channel| channel.parked.take());
                        child
                    }
                };
                *state = ScopeState::Running { channel, child };
                Ok(StepOutcome::InProgress)
            }
            ScopeState::Running { channel, mut child } => {
                if frame.device.active.is_none() {
                    match frame.device.command_request(channel) {
                        Ok(Some(CommandRequest::Fetch(_) | CommandRequest::Progress(_))) => {}
                        Ok(None | Some(CommandRequest::Resume(_))) => {
                            return Ok(StepOutcome::Complete(Ok(())));
                        }
                        Err(error) => return Ok(StepOutcome::Complete(Err(error))),
                    }
                }
                match self.0.step(frame.device, &mut child) {
                    Ok(StepOutcome::InProgress) => {
                        let Some(owner) = frame.device.channels.get_mut(channel.index()) else {
                            return Ok(StepOutcome::Complete(Err(Error::Routing)));
                        };
                        if owner.registers.values.fault_status.faulted() {
                            let Some(slot) = frame.saved.get_mut(channel.index()) else {
                                return Ok(StepOutcome::Complete(Err(Error::Routing)));
                            };
                            *slot = Some(child);
                            owner.parked = frame.device.active.take();
                            Ok(StepOutcome::Complete(Ok(())))
                        } else {
                            *state = ScopeState::Running { channel, child };
                            Ok(StepOutcome::InProgress)
                        }
                    }
                    Ok(StepOutcome::Complete(result)) => {
                        drop(child);
                        let halted = frame.device.halted();
                        if let Some(owner) = frame.device.channels.get_mut(channel.index()) {
                            if result.is_err() || halted || owner.resetting() || owner.halted() {
                                owner.progress = None;
                            }
                        }
                        if frame
                            .device
                            .channels
                            .get(channel.index())
                            .is_none_or(|owner| owner.progress.is_none())
                        {
                            frame.device.active = None;
                        }
                        Ok(StepOutcome::Complete(match result {
                            Ok(()) | Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted) => {
                                Ok(())
                            }
                            Err(error) => Err(error),
                        }))
                    }
                    Err(error) => {
                        drop(child);
                        frame.device.active = None;
                        Err(error)
                    }
                }
            }
            ScopeState::Finished => Err(RunError::Exhausted),
        }
    }
}
