use super::Transfer;
use crate::MemoryFailure;
use crate::channel::PayloadAccess;
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::ResponseStatus;

impl Transfer {
    pub(crate) fn read<M: MemoryAccess>(&mut self, memory: &M) -> PayloadAccess
    where
        M::Error: Into<MemoryFailure>,
    {
        if self.progress.status != ResponseStatus::Success || self.length == 0 {
            return PayloadAccess::Complete;
        }
        let Some(bytes) = self.bytes.get_mut(..self.length) else {
            self.progress.status = ResponseStatus::InvalidLength;
            return PayloadAccess::Complete;
        };
        if let Err(error) = memory.read(self.source, MemoryAlignment::ONE, bytes) {
            match error.into() {
                MemoryFailure::PageFault { .. } => return PayloadAccess::Fault,
                MemoryFailure::Access => self.progress.status = ResponseStatus::ReadError,
            }
        }
        PayloadAccess::Complete
    }

    pub(crate) fn write<M: MemoryAccess>(&mut self, memory: &M) -> PayloadAccess
    where
        M::Error: Into<MemoryFailure>,
    {
        if self.progress.status != ResponseStatus::Success {
            return PayloadAccess::Complete;
        }
        let (Some(bytes), Ok(count)) = (self.bytes.get(..self.length), u64::try_from(self.length))
        else {
            self.progress.status = ResponseStatus::InvalidLength;
            return PayloadAccess::Complete;
        };
        if count > self.progress.remaining
            || count > self.progress.source.remaining()
            || count > self.progress.destination.remaining()
            || (count == 0 && self.progress.remaining != 0)
        {
            self.progress.status = ResponseStatus::InvalidLength;
            return PayloadAccess::Complete;
        }
        if count == 0 {
            return PayloadAccess::Complete;
        }
        match memory.write(self.destination, MemoryAlignment::ONE, bytes) {
            Ok(()) => {
                self.progress.source.advance(count);
                self.progress.destination.advance(count);
                self.progress.remaining -= count;
                self.progress.bytes_completed += count;
            }
            Err(error) => match error.into() {
                MemoryFailure::PageFault { .. } => return PayloadAccess::Fault,
                MemoryFailure::Access => self.progress.status = ResponseStatus::WriteError,
            },
        }
        PayloadAccess::Complete
    }
}
