use crate::channel::RangeCheck;
use crate::{ChunkInput, CopyProgress, ExecutionContext, FetchedCommand, Flow, Work};
use hardware_interface_memory::MemoryAccess;
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};

/// Incrementally validates the freshly fetched descriptor exactly once.
#[derive(Debug, Default)]
pub struct CheckRanges {
    state: Option<Flow<(Work, RangeCheck)>>,
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for CheckRanges {
    type Input = Flow<FetchedCommand>;
    type Output = Flow<CopyProgress>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.state.is_none() {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            let work = input.work;
            let value = input.value.and_then(|value| {
                value.map_or(Ok(None), |fetched| match fetched.command {
                    pdma_model_contract::dma::Command::Copy(copy) => match fetched.admission {
                        crate::CommandAdmission::Resume => Ok(None),
                        crate::CommandAdmission::Fresh => {
                            Ok(Some((fetched.work, RangeCheck::new(copy))))
                        }
                    },
                    pdma_model_contract::dma::Command::Write(_)
                    | pdma_model_contract::dma::Command::Fill(_)
                    | pdma_model_contract::dma::Command::Wait(_)
                    | pdma_model_contract::dma::Command::Atomic(_)
                    | pdma_model_contract::dma::Command::Semaphore(_)
                    | pdma_model_contract::dma::Command::Fence(_)
                    | pdma_model_contract::dma::Command::Trap(_)
                    | pdma_model_contract::dma::Command::Timestamp(_) => Err(crate::Error::Routing),
                })
            });
            self.state = Some(Flow::new(work, value));
        }
        Flow::advance(&mut self.state, context, |state, context| {
            let Some((work, scan)) = state.as_mut() else {
                return Ok(StepOutcome::Complete(Ok(None)));
            };
            match context.ready(*work) {
                Ok(true) => {}
                Ok(false) => return Ok(StepOutcome::InProgress),
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error)));
                }
            }
            let work = *work;
            scan.advance(&context.memory_for(work, scan.command.sg_context))
                .map_or(Ok(StepOutcome::InProgress), |progress| {
                    if progress.status.is_err() {
                        let event = progress.event().ok_or(RunError::Exhausted)?;
                        *state = None;
                        return Ok(StepOutcome::Complete(Err(context.report(&event))));
                    }
                    let output = CopyProgress { work, progress };
                    *state = None;
                    Ok(StepOutcome::Complete(Ok(Some(output))))
                })
        })
    }
}

#[derive(Debug)]
pub(in crate::stages) struct Install;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Install {
    type Input = Flow<CopyProgress>;
    type Output = Flow<ChunkInput>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(input) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        Ok(StepOutcome::Complete(Ok(context.install_progress(input))))
    }
}
