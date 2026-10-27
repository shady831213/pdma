mod atomic;
mod fill;
pub(super) mod operand;
mod word;

use crate::{Completion, ExecutionContext, FetchedCommand, Flow, MemoryFailure};
use hardware_interface_memory::MemoryAccess;
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::{Event, EventCode};

#[derive(Debug)]
enum Failure {
    Fault,
    Stop(EventCode),
}

#[derive(Debug)]
enum Progress {
    Done,
    Yield,
    More,
}

#[derive(Debug)]
pub(super) struct Execute<T> {
    state: Option<Flow<T>>,
}

impl<T> Default for Execute<T> {
    fn default() -> Self {
        Self { state: None }
    }
}

impl<M: MemoryAccess<SB>, SB, T: operand::Operand> Stage<ExecutionContext<'_, M, SB>> for Execute<T>
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Flow<FetchedCommand>;
    type Output = Flow<Completion>;
    type Error = std::convert::Infallible;

    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.state.is_none() {
            self.state = input.take().map(|flow| {
                let value = flow.value.and_then(|fetched| {
                    fetched
                        .map(|fetched| T::project(fetched.command))
                        .transpose()
                });
                Flow::new(flow.work, value)
            });
        }
        Flow::advance(&mut self.state, context, |state, context| {
            let Some(operand) = state.as_ref().copied() else {
                return Ok(StepOutcome::Complete(Ok(None)));
            };
            let work = context.work();
            match context.ready(work) {
                Ok(true) => {}
                Ok(false) => return Ok(StepOutcome::InProgress),
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error)));
                }
            }
            let result = operand.execute(context);
            match result {
                Ok(Progress::More) => return Ok(StepOutcome::InProgress),
                Ok(Progress::Yield) => {
                    *context.active = None;
                    context.changed();
                    *state = None;
                    return Ok(StepOutcome::Complete(Ok(None)));
                }
                Err(Failure::Fault) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(context.park(work).map(|()| None)));
                }
                Ok(Progress::Done) => {}
                Err(Failure::Stop(code)) => {
                    let bytes_completed = context
                        .selected
                        .work
                        .as_ref()
                        .map_or(0, |accepted| accepted.fill_completed);
                    let event = Event::stopping(operand.id(), code, bytes_completed);
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(context.report(&event))));
                }
            }
            *state = None;
            Ok(StepOutcome::Complete(Ok(Some(Completion { work }))))
        })
    }
}

fn access_failure(error: impl Into<MemoryFailure>, code: EventCode) -> Failure {
    match error.into() {
        MemoryFailure::PageFault { .. } => Failure::Fault,
        MemoryFailure::Access => Failure::Stop(code),
    }
}
