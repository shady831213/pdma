use super::{CopyState, Cursor};
use crate::channel::{RangeCheck, Side};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{CopyCommand, EventCode};

impl RangeCheck {
    pub(crate) const fn new(id: u64, command: CopyCommand) -> Self {
        Self {
            cursor: Cursor::new(command.source, command.copy_length),
            remaining: command.copy_length,
            command,
            id,
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
                Side::Destination => Ok(()),
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
                        Err(EventCode::InvalidLength)
                    } else {
                        scan.cursor.advance(count);
                        scan.remaining -= count;
                        return None;
                    }
                }
                Ok(false) => Err(EventCode::LengthMismatch),
                Err(status) => Err(status),
            }
        };
        Some(CopyState::new(scan.id, scan.command, result))
    }
}
