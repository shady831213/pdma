use super::{CopyState, CopyStatus, Cursor};
use crate::channel::{RangeCheck, Side};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::Command;

impl RangeCheck {
    pub(crate) const fn new(command: Command) -> Option<Self> {
        let Some(copy) = command.as_copy() else {
            return None;
        };
        Some(Self {
            cursor: Cursor::new(copy.source, copy.copy_length),
            remaining: copy.copy_length,
            command: *copy,
            id: command.id,
            flags: command.flags,
            side: Side::Source,
        })
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
                Side::Destination => CopyStatus::Success,
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
                        CopyStatus::InvalidLength
                    } else {
                        scan.cursor.advance(count);
                        scan.remaining -= count;
                        return None;
                    }
                }
                Ok(false) => CopyStatus::LengthMismatch,
                Err(status) => status,
            }
        };
        Some(CopyState::new(scan, result))
    }
}
