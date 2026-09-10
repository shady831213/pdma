use super::{CopyState, Cursor};
use crate::{Device, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::{Command, ResponseStatus};

#[derive(Debug)]
enum Side {
    Source,
    Destination,
}

/// Incremental list-length scan; no payload memory is accessed.
#[derive(Debug)]
pub struct LengthCheck {
    command: Command,
    cursor: Cursor,
    side: Side,
    source_total: u64,
    total: u64,
}

/// Validate matching endpoint totals before copying any payload.
#[derive(Debug, Default)]
pub struct CheckLengths;

impl<M: MemoryAccess> Stage<Device<M>> for CheckLengths {
    type Input = Command;
    type Output = CopyState;
    type Error = Error;
    type Wait = ();
    type State = Option<LengthCheck>;

    fn start(command: Command) -> Self::State {
        Some(LengthCheck {
            cursor: Cursor::new(command.source),
            command,
            side: Side::Source,
            source_total: 0,
            total: 0,
        })
    }

    fn step(
        &mut self,
        context: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<CopyState, Error, ()> {
        if context.channel.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let Some(scan) = state.as_mut() else {
            return StepOutcome::Exhausted;
        };
        let result = match scan.cursor.next_segment(&context.memory) {
            Ok(Some(segment)) => match scan.total.checked_add(segment.length) {
                Some(total) => {
                    scan.total = total;
                    return StepOutcome::Progressed;
                }
                None => ResponseStatus::InvalidLength,
            },
            Ok(None) => match scan.side {
                Side::Source => {
                    scan.source_total = scan.total;
                    scan.total = 0;
                    scan.cursor = Cursor::new(scan.command.destination);
                    scan.side = Side::Destination;
                    return StepOutcome::Progressed;
                }
                Side::Destination => {
                    if scan.total == scan.source_total {
                        ResponseStatus::Success
                    } else {
                        ResponseStatus::LengthMismatch
                    }
                }
            },
            Err(status) => status,
        };
        let output = CopyState::new(scan.command, scan.source_total, result);
        *state = None;
        StepOutcome::Complete(Ok(output))
    }
}
