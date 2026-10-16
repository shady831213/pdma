use crate::{Dma, Error, ExecutionContext, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};

#[derive(Debug)]
pub struct Selected<S>(pub S);

impl<M, SB, S, I, T> Stage<Dma<M, SB>> for Selected<S>
where
    for<'a> S: Stage<ExecutionContext<'a, M, SB>, Input = I, Output = (), Error = Error, State = T>,
{
    type Input = Option<(Work, I)>;
    type Output = ();
    type Error = Error;
    type State = Option<Option<(Work, T)>>;
    fn start(input: Self::Input) -> Self::State {
        Some(input.map(|(work, input)| (work, S::start(input))))
    }
    fn step(&mut self, dma: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        let Some((work, local)) = state.as_mut().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(())));
        };
        let identity = *work;
        if let Some(error) = dma.invocation_error(identity) {
            *state = None;
            dma.release_invocation(identity);
            return Ok(StepOutcome::Complete(Err(error)));
        }
        let mut context = match dma.execution_context(identity.channel()) {
            Ok(context) => context,
            Err(error) => {
                *state = None;
                dma.release_invocation(identity);
                return Ok(StepOutcome::Complete(Err(error)));
            }
        };
        context.generation = identity.generation;
        let result = self.0.step(&mut context, local);
        if !matches!(result, Ok(StepOutcome::InProgress)) {
            *state = None;
            dma.release_invocation(identity);
        }
        result
    }
}
