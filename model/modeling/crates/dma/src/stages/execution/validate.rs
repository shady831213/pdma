use crate::channel::RangeCheck;
use crate::{ChunkInput, CopyProgress, ExecutionContext, FetchedCommand, Flow, Work};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;

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
            self.state = Some(Flow::new(
                input.work,
                input.value.and_then(|fetched| {
                    fetched
                        .map(|fetched| {
                            RangeCheck::new(fetched.command)
                                .map(|scan| (fetched.work, scan))
                                .ok_or(crate::Error::Routing)
                        })
                        .transpose()
                }),
            ));
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
        })
    }
}

#[derive(Debug)]
pub struct Install;

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
        let mut state = Some(input);
        Flow::advance(&mut state, context, |input, context| {
            let result = match input.take() {
                Some(progress) => context.install_progress(progress).map(Some),
                None => Ok(context
                    .active
                    .filter(|work| *work == context.work())
                    .map(ChunkInput::next)),
            };
            Ok(StepOutcome::Complete(result))
        })
    }
}
