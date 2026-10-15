use crate::{BranchError, RunError, Stage, StepOutcome, StepResult};
mod plan;
mod state;
pub use plan::{FanPlan, FanPolicy};
pub use state::FanState;
use state::Phase;

/// Finite N-to-M value flow with independent child progress and bounded buffering.
///
/// Each input is invoked once; output slots may be reused after completion.
/// Planning consumes selected completed values only after validating the full plan.
#[derive(Debug)]
pub struct Fan<S, P, T> {
    inputs: Vec<S>,
    policy: P,
    outputs: Vec<T>,
}
impl<S, P, T> Fan<S, P, T> {
    /// Owns input stages, value-selection policy, and output stages.
    pub const fn new(inputs: Vec<S>, policy: P, outputs: Vec<T>) -> Self {
        Self {
            inputs,
            policy,
            outputs,
        }
    }
}
impl<C, S, P, T> Stage<C> for Fan<S, P, T>
where
    S: Stage<C>,
    T: Stage<C, Error = S::Error>,
    P: FanPolicy<S::Output, T::Input, S::Error>,
{
    type Input = Vec<S::Input>;
    type Output = Vec<(usize, T::Output)>;
    type Error = BranchError<S::Error>;
    type State = Option<FanState<S::State, S::Output, T::State, T::Output>>;
    fn start(inputs: Self::Input) -> Self::State {
        Some(FanState::new(inputs.into_iter().map(S::start).collect()))
    }
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        let mut batch = state.take().ok_or(RunError::Exhausted)?;
        if batch.inputs.len() != self.inputs.len() {
            return Ok(StepOutcome::Complete(Err(BranchError::InputCount {
                expected: self.inputs.len(),
                actual: batch.inputs.len(),
            })));
        }
        if batch.outputs.is_empty() {
            batch.outputs = (0..self.outputs.len()).map(|_| None).collect();
        }
        let Some(phase) = batch.ready_phase() else {
            return Ok(StepOutcome::Complete(Ok(batch.receipts)));
        };
        match phase {
            Phase::Inputs => {
                for ((port, input), value) in self
                    .inputs
                    .iter_mut()
                    .zip(&mut batch.inputs)
                    .zip(&mut batch.values)
                {
                    if let Some(mut child) = input.take() {
                        match port.step(context, &mut child)? {
                            StepOutcome::InProgress => *input = Some(child),
                            StepOutcome::Complete(Ok(output)) => *value = Some(output),
                            StepOutcome::Complete(Err(error)) => {
                                return Ok(StepOutcome::Complete(Err(BranchError::Application(
                                    error,
                                ))));
                            }
                        }
                    }
                }
                batch.phase = Phase::Plan;
            }
            Phase::Plan => {
                let available: Vec<_> = batch.outputs.iter().map(Option::is_none).collect();
                let result = self
                    .policy
                    .plan(&batch.values, &available)
                    .map_err(BranchError::Application)
                    .and_then(|plan| plan.validate(&batch.values, &available));
                let plan = match result {
                    Ok(plan) => plan,
                    Err(error) => return Ok(StepOutcome::Complete(Err(error))),
                };
                if !plan.inputs.is_empty() {
                    let mut selected = Vec::with_capacity(plan.inputs.len());
                    for index in plan.inputs {
                        let value = batch
                            .values
                            .get_mut(index)
                            .and_then(Option::take)
                            .ok_or(RunError::Exhausted)?;
                        selected.push(value);
                    }
                    let routed = match self.policy.route(selected) {
                        Ok(routed) => routed,
                        Err(error) => {
                            return Ok(StepOutcome::Complete(Err(BranchError::Application(error))));
                        }
                    };
                    if routed.len() != plan.outputs.len() {
                        return Ok(StepOutcome::Complete(Err(BranchError::OutputCount {
                            expected: plan.outputs.len(),
                            actual: routed.len(),
                        })));
                    }
                    for (index, input) in plan.outputs.into_iter().zip(routed) {
                        let output = batch.outputs.get_mut(index).ok_or(RunError::Exhausted)?;
                        *output = Some(T::start(input));
                    }
                }
                batch.phase = Phase::Outputs;
            }
            Phase::Outputs => {
                for (index, (port, output)) in
                    self.outputs.iter_mut().zip(&mut batch.outputs).enumerate()
                {
                    if let Some(mut child) = output.take() {
                        match port.step(context, &mut child)? {
                            StepOutcome::InProgress => *output = Some(child),
                            StepOutcome::Complete(Ok(value)) => batch.receipts.push((index, value)),
                            StepOutcome::Complete(Err(error)) => {
                                return Ok(StepOutcome::Complete(Err(BranchError::Application(
                                    error,
                                ))));
                            }
                        }
                    }
                }
                batch.phase = Phase::Inputs;
            }
        }
        if batch.ready_phase().is_none() {
            return Ok(StepOutcome::Complete(Ok(batch.receipts)));
        }
        *state = Some(batch);
        Ok(StepOutcome::InProgress)
    }
}
