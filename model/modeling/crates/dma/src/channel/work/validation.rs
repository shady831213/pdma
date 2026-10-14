use super::{CopyState, Cursor};
use crate::channel::{RangeCheck, Side};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{Command, ResponseStatus};

impl RangeCheck {
    pub(crate) const fn new(command: Command) -> Self {
        Self {
            cursor: Cursor::new(command.source, command.copy_length),
            remaining: command.copy_length,
            command,
            side: Side::Source,
        }
    }
}

impl RangeCheck {
    pub(crate) fn advance<M: MemoryAccess>(&mut self, memory: &M) -> Option<CopyState> {
        let scan = self;
        let result = if scan.remaining == 0 {
            match scan.side {
                Side::Source => {
                    scan.cursor = Cursor::new(scan.command.destination, scan.command.copy_length);
                    scan.remaining = scan.command.copy_length;
                    scan.side = Side::Destination;
                    return None;
                }
                Side::Destination => ResponseStatus::Success,
            }
        } else {
            match scan.cursor.next_segment(memory) {
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
                        return None;
                    }
                }
                Ok(false) => ResponseStatus::LengthMismatch,
                Err(status) => status,
            }
        };
        Some(CopyState::new(scan.command, result))
    }
}
