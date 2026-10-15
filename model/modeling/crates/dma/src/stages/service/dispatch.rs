use super::super::{Commands, Fresh, WriteCopy};
use super::{AdmissionState, FreshState, Service, ServiceState};
use crate::channel::Continuation;
use crate::{
    ChunkInput, CommandRequest, Dma, Error, ExecutionContext, ExecutionError, MemoryFailure, Work,
};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::MemoryAccess;

impl<A, B> Service<A, B> {
    fn resume<M, SB, T, R, P>(
        device: &mut Dma<M, SB>,
        work: Work,
    ) -> Result<ServiceState<AdmissionState, FreshState, T, R, P>, Error>
    where
        for<'a> Then<A, Then<WriteCopy, B>>:
            Stage<ExecutionContext<'a, M, SB>, Input = Option<ChunkInput>, State = T>,
        for<'a> Then<WriteCopy, B>:
            Stage<ExecutionContext<'a, M, SB>, Input = Option<crate::Payload>, State = R>,
    {
        let mut context = device.execution_context(work.channel())?;
        let continuation = context
            .channel(work)?
            .work
            .as_mut()
            .and_then(|accepted| accepted.continuation.take())
            .ok_or(Error::Routing)?;
        *context.active = Some(work);
        Ok(match continuation {
            Continuation::Next => ServiceState::Execution(
                work.channel(),
                <Then<A, Then<WriteCopy, B>> as Stage<ExecutionContext<'_, M, SB>>>::start(Some(
                    ChunkInput::next(work),
                )),
            ),
            Continuation::Read(chunk) => ServiceState::Execution(
                work.channel(),
                <Then<A, Then<WriteCopy, B>> as Stage<ExecutionContext<'_, M, SB>>>::start(Some(
                    ChunkInput::retry(chunk),
                )),
            ),
            Continuation::Write(payload) => ServiceState::RetryWrite(
                work.channel(),
                <Then<WriteCopy, B> as Stage<ExecutionContext<'_, M, SB>>>::start(Some(*payload)),
            ),
        })
    }

    pub(super) fn advance<M: MemoryAccess<SB>, SB, T, R, P>(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut ServiceState<AdmissionState, FreshState, T, R, P>,
    ) -> StepResult<(), ExecutionError>
    where
        M::Error: Into<MemoryFailure>,
        for<'a> Then<A, Then<WriteCopy, B>>: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<ChunkInput>,
                Output = (),
                Error = ExecutionError,
                State = T,
            >,
        for<'a> Then<WriteCopy, B>: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<crate::Payload>,
                Output = (),
                Error = ExecutionError,
                State = R,
            >,
        for<'a> B: Stage<
                ExecutionContext<'a, M, SB>,
                Input = Option<crate::Completion>,
                Output = (),
                Error = ExecutionError,
                State = P,
            >,
    {
        match state {
            ServiceState::Select => {
                *state = match device.active {
                    Some(work) => match Self::resume(device, work) {
                        Ok(next) => next,
                        Err(error) => return Ok(StepOutcome::Complete(Err(error.into()))),
                    },
                    None => ServiceState::Admission(<Commands as Stage<Dma<M, SB>>>::start(
                        vec![(); self.count],
                    )),
                };
            }
            ServiceState::Admission(local) => match self.command.step(device, local)? {
                StepOutcome::InProgress => {}
                StepOutcome::Complete(Ok(requests)) => {
                    let Some(request) = requests.into_iter().find_map(|(_, request)| request)
                    else {
                        return Ok(StepOutcome::Complete(Ok(())));
                    };
                    let Some(request) = device
                        .command_request(request.channel())
                        .map_err(|_| RunError::Exhausted)?
                    else {
                        return Ok(StepOutcome::Complete(Ok(())));
                    };
                    *state = match request {
                        CommandRequest::Fetch(snapshot) => ServiceState::Fresh(
                            request.channel(),
                            <Fresh as Stage<ExecutionContext<'_, M, SB>>>::start(Some(snapshot)),
                        ),
                        CommandRequest::Resume(work) => match Self::resume(device, work) {
                            Ok(next) => next,
                            Err(error) => return Ok(StepOutcome::Complete(Err(error.into()))),
                        },
                    };
                }
                StepOutcome::Complete(Err(error)) => {
                    return Ok(StepOutcome::Complete(Err(Error::from(error).into())));
                }
            },
            ServiceState::Fresh(id, local) => {
                let mut context = match device.execution_context(*id) {
                    Ok(context) => context,
                    Err(error) => return Ok(StepOutcome::Complete(Err(error.into()))),
                };
                match self.fresh.step(&mut context, local)? {
                    StepOutcome::InProgress => {}
                    StepOutcome::Complete(Ok(Some(progress))) => {
                        let work = progress.work;
                        if let Err(error) = context
                            .install_progress(progress)
                            .and_then(|_| context.retain(work, Continuation::Next))
                        {
                            return Ok(StepOutcome::Complete(Err(error.into())));
                        }
                        return Ok(StepOutcome::Complete(Ok(())));
                    }
                    StepOutcome::Complete(Ok(None)) => return Ok(StepOutcome::Complete(Ok(()))),
                    StepOutcome::Complete(Err(error)) => {
                        return Ok(StepOutcome::Complete(Err(error)));
                    }
                }
            }
            ServiceState::Execution(id, local) => {
                return match device.execution_context(*id) {
                    Ok(mut context) => self.execution.step(&mut context, local),
                    Err(error) => Ok(StepOutcome::Complete(Err(error.into()))),
                };
            }
            ServiceState::Completion(id, local) => {
                return match device.execution_context(*id) {
                    Ok(mut context) => self
                        .execution
                        .downstream
                        .downstream
                        .step(&mut context, local),
                    Err(error) => Ok(StepOutcome::Complete(Err(error.into()))),
                };
            }
            ServiceState::RetryWrite(id, local) => {
                return match device.execution_context(*id) {
                    Ok(mut context) => self.execution.downstream.step(&mut context, local),
                    Err(error) => Ok(StepOutcome::Complete(Err(error.into()))),
                };
            }
        }
        Ok(StepOutcome::InProgress)
    }
}
