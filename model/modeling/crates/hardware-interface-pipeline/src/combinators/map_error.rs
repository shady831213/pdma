use crate::{ReadyStage, Stage, StepOutcome};

/// Changes a stage's error type while preserving its operation state.
#[derive(Debug)]
pub struct MapError<S, F> {
    stage: S,
    map: F,
}

impl<S, F> MapError<S, F> {
    /// Wraps existing hardware with an application error conversion.
    pub const fn new(stage: S, map: F) -> Self {
        Self { stage, map }
    }
}

impl<C, S, F, E> Stage<C> for MapError<S, F>
where
    S: Stage<C>,
    F: FnMut(S::Error) -> E,
{
    type Input = S::Input;
    type Output = S::Output;
    type Error = E;
    type Wait = S::Wait;
    type State = S::State;
    fn start(input: Self::Input) -> Self::State {
        S::start(input)
    }
    fn step(
        &mut self,
        context: &mut C,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, E, Self::Wait> {
        match self.stage.step(context, state) {
            StepOutcome::Progressed(state) => StepOutcome::Progressed(state),
            StepOutcome::Pending(state, wait) => StepOutcome::Pending(state, wait),
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map_err(&mut self.map)),
        }
    }
}

impl<C, S, F, E> ReadyStage<C> for MapError<S, F>
where
    S: ReadyStage<C>,
    F: FnMut(S::Error) -> E,
{
    fn ready(&self, context: &C) -> bool {
        self.stage.ready(context)
    }
}
