use crate::{Completion, Dma, Error, Flow};
use hw_model_pipeline::{Stage, StepOutcome, StepResult};

#[cfg(test)]
mod tests;

/// Consumes all completed branch outcomes and releases their resident owners.
#[derive(Debug)]
pub struct Retire;

impl<M, SB> Stage<Dma<M, SB>> for Retire {
    type Input = Vec<(usize, Flow<Completion>)>;
    type Output = ();
    type Error = Error;

    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<(), Error> {
        let Some(outputs) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let mut result = Ok(());
        for (_, flow) in outputs {
            let outcome = flow.lifecycle(&mut dma.execution_context()).and_then(|()| {
                if let Some(completion) = flow.data() {
                    if flow.work != Some(completion.work) {
                        return Err(Error::Routing);
                    }
                    let mut context = dma.execution_context();
                    let selected = context.select(completion.work)?;
                    if selected.selected.resident != Some(completion.work) {
                        return Err(Error::Routing);
                    }
                }
                Ok(())
            });
            if let Some(work) = flow.work {
                if flow.data().is_some() || outcome.is_err() {
                    dma.retire_accepted(work);
                }
                dma.release_residency(work);
            }
            result = result.and(outcome);
        }
        Ok(StepOutcome::Complete(result))
    }
}
