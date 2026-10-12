mod control;

pub use control::ResetControl;

use crate::{ChannelContext, Dma, Error};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::ChannelId;

/// A value bound to the channel generation that accepted its command.
#[derive(Debug)]
pub struct ChannelTask<T> {
    pub(crate) channel: ChannelId,
    pub(crate) generation: u64,
    /// Typed payload passed into the selected channel's composition.
    pub value: T,
}

impl<T> ChannelTask<T> {
    /// Identifies the channel owning this task.
    pub const fn channel(&self) -> ChannelId {
        self.channel
    }

    /// Transforms the payload while retaining the original reset boundary.
    pub fn map<U>(self, transform: impl FnOnce(T) -> U) -> ChannelTask<U> {
        ChannelTask {
            channel: self.channel,
            generation: self.generation,
            value: transform(self.value),
        }
    }
}

/// Routes one accepted task through a composition and rejects work invalidated by reset.
#[derive(Debug)]
pub struct Resettable<S> {
    stage: S,
    channel: Option<ChannelId>,
}

impl<S> Resettable<S> {
    /// Wraps hardware whose context is the task's selected channel.
    pub const fn new(stage: S) -> Self {
        Self {
            stage,
            channel: None,
        }
    }

    /// Binds a destination port to one channel, rejecting misrouted tasks before effects.
    pub const fn for_channel(channel: ChannelId, stage: S) -> Self {
        Self {
            stage,
            channel: Some(channel),
        }
    }
}

impl<M, S, I, O, T, SB> Stage<Dma<M, SB>> for Resettable<S>
where
    for<'a> S: Stage<ChannelContext<'a, M, SB>, Input = I, Output = O, State = T, Error = Error>,
{
    type Input = ChannelTask<I>;
    type Output = ChannelTask<O>;
    type Error = Error;
    type State = Option<ChannelTask<T>>;

    fn start(input: Self::Input) -> Self::State {
        Some(input.map(S::start))
    }

    fn step(
        &mut self,
        context: &mut Dma<M, SB>,
        retained: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let mut state = retained.take().ok_or(RunError::Exhausted)?;
        if self.channel.is_some_and(|channel| channel != state.channel) {
            return Ok(StepOutcome::Complete(Err(Error::Routing)));
        }
        let mut selected = match context.channel_context(state.channel) {
            Ok(selected) => selected,
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
        };
        if state.generation != selected.channel.generation {
            return Ok(StepOutcome::Complete(Err(Error::Reset)));
        }
        if selected.halted() {
            return Ok(StepOutcome::Complete(Err(Error::Halted)));
        }
        Ok(match self.stage.step(&mut selected, &mut state.value)? {
            StepOutcome::Complete(result) => {
                StepOutcome::Complete(result.map(|value| ChannelTask {
                    channel: state.channel,
                    generation: state.generation,
                    value,
                }))
            }
            StepOutcome::InProgress => {
                *retained = Some(state);
                StepOutcome::InProgress
            }
        })
    }
}
