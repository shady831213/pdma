use super::{Fan, FanState};
use crate::{BranchError, RunError, Stage, StepOutcome, StepResult};

impl<S, P, T, I, V, J, O, E> Fan<S, P, T, I, V, J, O, E> {
    pub(super) fn settle<C>(
        &mut self,
        context: &mut C,
        batch: &mut FanState<I, V, J, O, E>,
    ) -> StepResult<(), BranchError<E>>
    where
        S: Stage<C, Input = I, Output = V, Error = E>,
        T: Stage<C, Input = J, Output = O, Error = E>,
    {
        settle_ports(&mut self.inputs, &mut batch.input_active, context)?;
        settle_ports(&mut self.outputs, &mut batch.output_active, context)?;
        if batch.ready_phase().is_some() {
            return Ok(StepOutcome::InProgress);
        }
        Ok(StepOutcome::Complete(Err(batch
            .failure
            .take()
            .ok_or(RunError::Exhausted)?)))
    }
}

fn settle_ports<C, S: Stage<C>>(
    ports: &mut [S],
    active: &mut [bool],
    context: &mut C,
) -> Result<(), RunError> {
    for (port, active) in ports.iter_mut().zip(active) {
        if *active && matches!(port.step(context, &mut None)?, StepOutcome::Complete(_)) {
            *active = false;
        }
    }
    Ok(())
}
