use crate::channel::RangeCheck;
use crate::{CopyProgress, ExecutionContext, FetchedCommand, Work};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

/// Incrementally validates the freshly fetched descriptor exactly once.
#[derive(Debug, Default)]
pub struct CheckRanges {
    state: Option<(Work, RangeCheck)>,
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for CheckRanges {
    type Input = Option<FetchedCommand>;
    type Output = Option<CopyProgress>;
    type Error = crate::Error;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, crate::Error> {
        if self.state.is_none() {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.state = input.map(|fetched| (fetched.work, RangeCheck::new(fetched.command)));
        }
        let state = &mut self.state;
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
                if progress.status.halts() {
                    *state = None;
                    return Ok(StepOutcome::Complete(
                        context
                            .complete(crate::Completion {
                                work,
                                response: progress.response(),
                            })
                            .map(|()| None),
                    ));
                }
                let output = CopyProgress { work, progress };
                *state = None;
                Ok(StepOutcome::Complete(Ok(Some(output))))
            })
    }
}
