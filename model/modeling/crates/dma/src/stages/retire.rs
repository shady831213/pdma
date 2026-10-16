use crate::{Dma, Error, Flow};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};

/// Consumes a completed execution outcome and releases its resident owner.
#[derive(Debug)]
pub struct Retire;

impl<M, SB> Stage<Dma<M, SB>> for Retire {
    type Input = Flow<()>;
    type Output = ();
    type Error = Error;

    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<(), Error> {
        let Some(flow) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let result = flow.status();
        if let Some(work) = flow.work {
            dma.release_residency(work);
        }
        Ok(StepOutcome::Complete(result))
    }
}
