use crate::channel::{Action, PayloadAccess};
use crate::{Dma, Error, MemoryFailure, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::ResponseStatus;

/// Reads a payload chunk into channel-owned scratch and parks repairable faults.
#[derive(Debug, Default)]
pub struct ReadCopy;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for ReadCopy
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Option<Work>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Option<Work>>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(work) = *state.as_ref().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        match device.work_ready(work) {
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
            Ok(false) => return Ok(StepOutcome::InProgress),
            Ok(true) => {}
        }
        let context = match device.channel_context(work.channel()) {
            Ok(context) => context,
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
        };
        let mut accepted = context.channel.work.take().ok_or(RunError::Exhausted)?;
        let mut faulted = false;
        accepted.action = match accepted.action {
            Action::Read(mut transfer) => {
                match transfer.read(&context.memory_for(transfer.progress.command.source.context)) {
                    PayloadAccess::Complete => Action::Transform(transfer),
                    PayloadAccess::Fault => {
                        faulted = true;
                        Action::Read(transfer)
                    }
                }
            }
            action @ (Action::Check(_)
            | Action::Next(_)
            | Action::Transform(_)
            | Action::Write(_)
            | Action::Respond(_)) => action,
        };
        context.channel.work = Some(accepted);
        if faulted {
            context.channel.suspend();
            device.active = None;
        }
        *state = None;
        Ok(StepOutcome::Complete(Ok(if faulted {
            None
        } else {
            Some(work)
        })))
    }
}

/// Writes retained scratch and advances channel traversal or prepares its response.
#[derive(Debug, Default)]
pub struct WriteCopy;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for WriteCopy
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Option<Work>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Option<Work>>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(work) = *state.as_ref().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        match device.work_ready(work) {
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
            Ok(false) => return Ok(StepOutcome::InProgress),
            Ok(true) => {}
        }
        let context = match device.channel_context(work.channel()) {
            Ok(context) => context,
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
        };
        let mut accepted = context.channel.work.take().ok_or(RunError::Exhausted)?;
        let mut faulted = false;
        accepted.action = match accepted.action {
            Action::Transform(mut transfer) | Action::Write(mut transfer) => {
                match transfer
                    .write(&context.memory_for(transfer.progress.command.destination.context))
                {
                    PayloadAccess::Fault => {
                        faulted = true;
                        Action::Write(transfer)
                    }
                    PayloadAccess::Complete
                        if transfer.progress.status == ResponseStatus::Success
                            && transfer.progress.remaining > 0 =>
                    {
                        Action::Next(transfer.progress)
                    }
                    PayloadAccess::Complete => Action::Respond(transfer.progress.response()),
                }
            }
            action
            @ (Action::Check(_) | Action::Next(_) | Action::Read(_) | Action::Respond(_)) => action,
        };
        let response_ready = matches!(accepted.action, Action::Respond(_));
        context.channel.work = Some(accepted);
        if faulted {
            context.channel.suspend();
            device.active = None;
        }
        *state = None;
        Ok(StepOutcome::Complete(Ok(if response_ready {
            Some(work)
        } else {
            None
        })))
    }
}
