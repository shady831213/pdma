use crate::{Device, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};

/// A channel reset boundary around an otherwise unchanged stage composition.
#[derive(Debug)]
pub struct Resettable<S>(S);

/// Retained stage state belongs to the channel generation that first executed it.
#[derive(Debug)]
pub struct ResetState<S> {
    generation: Option<u64>,
    inner: S,
}

impl<S> Resettable<S> {
    /// Wraps a composed channel pipeline without changing its typed ports.
    pub const fn new(stage: S) -> Self {
        Self(stage)
    }
}

impl<M, S> Stage<Device<M>> for Resettable<S>
where
    S: Stage<Device<M>, Error = Error>,
{
    type Input = S::Input;
    type Output = S::Output;
    type Error = Error;
    type Wait = S::Wait;
    type State = ResetState<S::State>;

    fn start(input: Self::Input) -> Self::State {
        ResetState {
            generation: None,
            inner: S::start(input),
        }
    }

    fn step(
        &mut self,
        context: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Error, Self::Wait> {
        let generation = state.generation.get_or_insert(context.channel.generation);
        if *generation != context.channel.generation {
            return StepOutcome::Complete(Err(Error::Reset));
        }
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        self.0.step(context, &mut state.inner)
    }
}
