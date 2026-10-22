mod cursor;
mod traversal;
mod validation;

use super::CopyState;
use crate::Channel;
pub(super) use cursor::Cursor;
use pdma_model_contract::dma::{Command, CopyCommand, Event, EventCode};

/// Scratch space per copy interaction, not a protocol transfer-length limit.
pub const COPY_CHUNK_BYTES: usize = 256;

impl CopyState {
    pub(crate) fn commit(&mut self, count: usize) -> Result<(), crate::Error> {
        let count = u64::try_from(count).map_err(|_| crate::Error::Routing)?;
        if count > self.remaining
            || count > self.source.remaining()
            || count > self.destination.remaining()
        {
            return Err(crate::Error::Routing);
        }
        self.source.advance(count);
        self.destination.advance(count);
        self.remaining -= count;
        self.bytes_completed += count;
        Ok(())
    }
    const fn new(id: u64, command: CopyCommand, status: Result<(), EventCode>) -> Self {
        Self {
            source: Cursor::new(command.source, command.copy_length),
            destination: Cursor::new(command.destination, command.copy_length),
            command,
            id,
            remaining: command.copy_length,
            bytes_completed: 0,
            status,
        }
    }

    pub(crate) fn event(&self) -> Option<Event> {
        self.status.err().map(|code| self.event_with(code))
    }

    pub(crate) const fn event_with(&self, code: EventCode) -> Event {
        Event::stopping(self.id, code, self.bytes_completed)
    }
}

impl Channel {
    pub(crate) fn accepted_command(&self) -> Option<&Command> {
        self.work.as_ref().map(|work| &work.command)
    }
}
