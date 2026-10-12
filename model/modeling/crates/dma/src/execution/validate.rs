use super::{CopyState, Cursor};
use crate::{ChannelContext, Error};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{Command, ResponseStatus};

#[derive(Debug)]
enum Side {
    Source,
    Destination,
}

/// Incremental validation of selected ranges, without payload access.
#[derive(Debug)]
pub struct RangeCheck {
    command: Command,
    cursor: Cursor,
    side: Side,
    remaining: u64,
}

/// Validate both selected ranges before committing payload writes.
#[derive(Debug, Default)]
pub struct CheckRanges;

impl<M: MemoryAccess<SB>, SB> Stage<ChannelContext<'_, M, SB>> for CheckRanges {
    type Input = Command;
    type Output = CopyState;
    type Error = Error;
    type State = Option<RangeCheck>;

    fn start(command: Command) -> Self::State {
        Some(RangeCheck {
            cursor: Cursor::new(command.source, command.copy_length),
            remaining: command.copy_length,
            command,
            side: Side::Source,
        })
    }

    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<CopyState, Error> {
        if context.channel.resetting() {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Reset)));
        }
        if context.halted() {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Halted)));
        }
        if !context.channel.enabled() {
            return Ok(StepOutcome::InProgress);
        }
        let Some(scan) = state.as_mut() else {
            return Err(RunError::Exhausted);
        };
        let result = if scan.remaining == 0 {
            match scan.side {
                Side::Source => {
                    scan.cursor = Cursor::new(scan.command.destination, scan.command.copy_length);
                    scan.remaining = scan.command.copy_length;
                    scan.side = Side::Destination;
                    return Ok(StepOutcome::InProgress);
                }
                Side::Destination => ResponseStatus::Success,
            }
        } else {
            match scan
                .cursor
                .next_segment(&context.memory_for(scan.command.sg_context))
            {
                Ok(true) => {
                    let count = scan.remaining.min(scan.cursor.remaining());
                    if count != 0
                        && scan
                            .cursor
                            .address()
                            .and_then(|address| address.checked_add(count))
                            .is_none()
                    {
                        ResponseStatus::InvalidLength
                    } else {
                        scan.cursor.advance(count);
                        scan.remaining -= count;
                        return Ok(StepOutcome::InProgress);
                    }
                }
                Ok(false) => ResponseStatus::LengthMismatch,
                Err(status) => status,
            }
        };
        let output = CopyState::new(scan.command, result);
        *state = None;
        Ok(StepOutcome::Complete(Ok(output)))
    }
}
