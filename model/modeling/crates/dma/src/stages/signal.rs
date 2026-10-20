mod operation;

use crate::{Completion, Error, ExecutionContext, FetchedCommand, Flow, MemoryFailure};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{AtomicAccessError, MemoryAccess, MemoryAddress};
use pdma_model_contract::dma::{
    CommandPayloadValues, Response, ResponseInfoValues, SignalErrorInfo, SignalErrorReason,
    SignalInfo,
};

#[derive(Debug)]
pub(super) struct Signals;

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Signals
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Flow<FetchedCommand>;
    type Output = Flow<FetchedCommand>;
    type Error = std::convert::Infallible;

    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        Flow::advance(input, context, |input, context| {
            let work = context.work();
            let command = match context.selected.accepted_command() {
                Some(command) => *command,
                None => return Ok(StepOutcome::Complete(Err(Error::Reset))),
            };
            let request = match command.payload {
                CommandPayloadValues::Copy(_) => {
                    return Ok(StepOutcome::Complete(Ok(input.take())));
                }
                CommandPayloadValues::Signal(signal) => operation::signal(signal),
                CommandPayloadValues::Wait(wait) => Ok(operation::wait(wait)),
            };
            match context.checkpoint_ready() {
                Ok(true) => {}
                Ok(false) => return Ok(StepOutcome::InProgress),
                Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            }
            if context
                .selected
                .work
                .as_ref()
                .is_some_and(|work| work.completion.is_some())
            {
                return Ok(StepOutcome::Complete(Ok(None)));
            }
            let result = match request {
                Err(reason) => Ok(ResponseInfoValues::SignalError(SignalErrorInfo { reason })),
                Ok(request) => {
                    if request.target.address % 8 != 0 {
                        Ok(ResponseInfoValues::SignalError(SignalErrorInfo {
                            reason: SignalErrorReason::InvalidOperation,
                        }))
                    } else {
                        match context.memory_for(work, request.target.context).atomic64(
                            MemoryAddress::new(request.target.address),
                            request.operation,
                        ) {
                            Ok(value) => {
                                if request.condition.is_some_and(|(condition, compare)| {
                                    !condition.matches(
                                        i64::from_le_bytes(value.to_le_bytes()),
                                        i64::from_le_bytes(compare.to_le_bytes()),
                                    )
                                }) {
                                    if let Some(accepted) = context.selected.work.as_mut() {
                                        accepted.waiting = true;
                                    }
                                    *context.active = None;
                                    context.changed();
                                    return Ok(StepOutcome::Complete(Ok(None)));
                                }
                                Ok(ResponseInfoValues::SignalSuccess(SignalInfo { value }))
                            }
                            Err(AtomicAccessError::Unsupported) => {
                                Ok(ResponseInfoValues::SignalError(SignalErrorInfo {
                                    reason: SignalErrorReason::Unsupported,
                                }))
                            }
                            Err(AtomicAccessError::Access(error)) => match error.into() {
                                MemoryFailure::PageFault { .. } => Err(()),
                                MemoryFailure::Access => {
                                    Ok(ResponseInfoValues::SignalError(SignalErrorInfo {
                                        reason: SignalErrorReason::Access,
                                    }))
                                }
                            },
                        }
                    }
                }
            };
            if let Some(accepted) = context.selected.work.as_mut() {
                accepted.waiting = false;
            }
            let result = match result {
                Ok(info) => context.complete(Completion {
                    work,
                    response: Response {
                        id: command.id,
                        flags: command.flags,
                        status: info.tag(),
                        info,
                    },
                }),
                Err(()) => context.park(work),
            };
            Ok(StepOutcome::Complete(result.map(|()| None)))
        })
    }
}
