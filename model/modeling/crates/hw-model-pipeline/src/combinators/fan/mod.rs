use crate::{BranchError, RunError, Stage, StepOutcome, StepResult};
mod plan;
mod settle;
mod state;
pub use plan::{FanPlan, FanPolicy};
use state::{FanState, Phase};

/// Finite N-to-M value flow with independent child progress and bounded buffering.
///
/// Each input is invoked once; output slots may be reused after completion.
/// Planning consumes selected completed values only after validating the full plan.
/// Failure stops admission and routing, then steps every accepted child once per
/// call until all have completed before delivering the first error.
#[derive(Debug)]
pub struct Fan<S, P, T, I, V, J, O, E> {
    inputs: Vec<S>,
    policy: P,
    outputs: Vec<T>,
    batch: Option<FanState<I, V, J, O, E>>,
}
impl<S, P, T, I, V, J, O, E> Fan<S, P, T, I, V, J, O, E> {
    pub(super) const fn is_active(&self) -> bool {
        self.batch.is_some()
    }

    /// Owns input stages, value-selection policy, and output stages.
    pub const fn new(inputs: Vec<S>, policy: P, outputs: Vec<T>) -> Self {
        Self {
            inputs,
            policy,
            outputs,
            batch: None,
        }
    }
}
impl<C, S, P, T, I, V, J, O, E> Stage<C> for Fan<S, P, T, I, V, J, O, E>
where
    S: Stage<C, Input = I, Output = V, Error = E>,
    T: Stage<C, Input = J, Output = O, Error = S::Error>,
    P: FanPolicy<S::Output, T::Input, S::Error>,
{
    type Input = Vec<S::Input>;
    type Output = Vec<(usize, T::Output)>;
    type Error = BranchError<S::Error>;
    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.batch.is_none() {
            let Some(inputs) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.batch = Some(FanState::new(inputs));
        }
        let mut batch = self.batch.take().ok_or(RunError::Exhausted)?;
        let result = if batch.failure.is_some() {
            self.settle(context, &mut batch)
        } else {
            self.advance(context, &mut batch)
        };
        match result {
            Ok(StepOutcome::Complete(Ok(()))) => Ok(StepOutcome::Complete(Ok(batch.receipts))),
            Ok(StepOutcome::Complete(Err(error))) => {
                batch.fail(error);
                if batch.ready_phase().is_none() {
                    Ok(StepOutcome::Complete(Err(batch
                        .failure
                        .take()
                        .ok_or(RunError::Exhausted)?)))
                } else {
                    self.batch = Some(batch);
                    Ok(StepOutcome::InProgress)
                }
            }
            other => {
                self.batch = Some(batch);
                other.map(|_| StepOutcome::InProgress)
            }
        }
    }
}
impl<S, P, T, I, V, J, O, E> Fan<S, P, T, I, V, J, O, E> {
    fn advance<C>(
        &mut self,
        context: &mut C,
        batch: &mut FanState<I, V, J, O, E>,
    ) -> StepResult<(), BranchError<E>>
    where
        S: Stage<C, Input = I, Output = V, Error = E>,
        T: Stage<C, Input = J, Output = O, Error = S::Error>,
        P: FanPolicy<V, J, S::Error>,
    {
        if batch.inputs.len() != self.inputs.len() {
            return Ok(StepOutcome::Complete(Err(BranchError::InputCount {
                expected: self.inputs.len(),
                actual: batch.inputs.len(),
            })));
        }
        batch.prepare_outputs(self.outputs.len());
        let Some(phase) = batch.ready_phase() else {
            return Ok(StepOutcome::Complete(Ok(())));
        };
        match phase {
            Phase::Inputs => {
                match self.advance_inputs(context, batch)? {
                    StepOutcome::Complete(Ok(())) | StepOutcome::InProgress => {}
                    StepOutcome::Complete(Err(error)) => {
                        return Ok(StepOutcome::Complete(Err(BranchError::Application(error))));
                    }
                }
                batch.phase = Phase::Plan;
            }
            Phase::Plan => {
                let available: Vec<_> = batch.output_active.iter().map(|active| !active).collect();
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
                        *output = Some(input);
                        *batch
                            .output_active
                            .get_mut(index)
                            .ok_or(RunError::Exhausted)? = true;
                    }
                }
                batch.phase = Phase::Outputs;
            }
            Phase::Outputs => {
                for (index, ((port, output), active)) in self
                    .outputs
                    .iter_mut()
                    .zip(&mut batch.outputs)
                    .zip(&mut batch.output_active)
                    .enumerate()
                {
                    if *active {
                        match port.step(context, output)? {
                            StepOutcome::InProgress => {}
                            StepOutcome::Complete(Ok(value)) => {
                                batch.receipts.push((index, value));
                                *active = false;
                            }
                            StepOutcome::Complete(Err(error)) => {
                                *active = false;
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
            return Ok(StepOutcome::Complete(Ok(())));
        }
        Ok(StepOutcome::InProgress)
    }
    fn advance_inputs<C>(
        &mut self,
        context: &mut C,
        batch: &mut FanState<I, V, J, O, E>,
    ) -> StepResult<(), S::Error>
    where
        S: Stage<C, Input = I, Output = V, Error = E>,
    {
        for (((port, input), value), active) in self
            .inputs
            .iter_mut()
            .zip(&mut batch.inputs)
            .zip(&mut batch.values)
            .zip(&mut batch.input_active)
        {
            if *active {
                match port.step(context, input)? {
                    StepOutcome::InProgress => {}
                    StepOutcome::Complete(Ok(output)) => {
                        *value = Some(output);
                        *active = false;
                    }
                    StepOutcome::Complete(Err(error)) => {
                        *active = false;
                        return Ok(StepOutcome::Complete(Err(error)));
                    }
                }
            }
        }

        Ok(StepOutcome::Complete(Ok(())))
    }
}
