use crate::channel::RangeCheck;
use crate::{CopyProgress, ExecutionContext, FetchedCommand, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

/// Incrementally validates the freshly fetched descriptor exactly once.
#[derive(Debug, Default)]
pub struct CheckRanges;

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for CheckRanges {
    type Input = Option<FetchedCommand>;
    type Output = Option<CopyProgress>;
    type Error = crate::ExecutionError;
    type State = Option<Option<(Work, RangeCheck)>>;
    fn start(input: Self::Input) -> Self::State {
        Some(input.map(|fetched| (fetched.work, RangeCheck::new(fetched.command))))
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, crate::ExecutionError> {
        let Some((work, scan)) = state.as_mut().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        match context.ready(*work) {
            Ok(true) => {}
            Ok(false) => return Ok(StepOutcome::InProgress),
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error.into())));
            }
        }
        let work = *work;
        scan.advance(&context.memory_for(work, scan.command.sg_context))
            .map_or(Ok(StepOutcome::InProgress), |progress| {
                if progress.status.halts() {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(
                        crate::ExecutionError::CommandFailed(crate::Completion {
                            work,
                            response: progress.response(),
                        }),
                    )));
                }
                let output = CopyProgress { work, progress };
                *state = None;
                Ok(StepOutcome::Complete(Ok(Some(output))))
            })
    }
}
