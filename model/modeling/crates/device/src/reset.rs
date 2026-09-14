use crate::{ChannelContext, Device, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::ChannelId;

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
}

impl<S> Resettable<S> {
    /// Wraps hardware whose context is the task's selected channel.
    pub const fn new(stage: S) -> Self {
        Self { stage }
    }
}

impl<M, S, I, O, W, T> Stage<Device<M>> for Resettable<S>
where
    for<'a> S:
        Stage<ChannelContext<'a, M>, Input = I, Output = O, Wait = W, State = T, Error = Error>,
{
    type Input = ChannelTask<I>;
    type Output = O;
    type Error = Error;
    type Wait = W;
    type State = ChannelTask<T>;

    fn start(input: Self::Input) -> Self::State {
        input.map(S::start)
    }

    fn step(
        &mut self,
        context: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<O, Error, W> {
        let mut selected = match context.channel_context(state.channel) {
            Ok(selected) => selected,
            Err(error) => return StepOutcome::Complete(Err(error)),
        };
        if state.generation != selected.channel.generation {
            return StepOutcome::Complete(Err(Error::Reset));
        }
        if selected.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        self.stage.step(&mut selected, &mut state.value)
    }
}
