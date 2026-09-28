use crate::ring::MemoryRingIo;
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::control::{ControlQueue, ControlRingState};
use pdma_model_contract::ring::RingError;

#[derive(Clone, Copy, Debug)]
struct Cursors {
    producer: MemoryAddress,
    consumer: MemoryAddress,
}

impl Cursors {
    fn new(base: MemoryAddress, ring: usize) -> Result<Self, RingError> {
        let field = |field: usize| {
            ring.checked_add(field)
                .and_then(|offset| u64::try_from(offset).ok())
                .and_then(|offset| base.raw().checked_add(offset))
                .map(MemoryAddress::new)
                .ok_or(RingError::Geometry)
        };
        Ok(Self {
            producer: field(ControlRingState::PRODUCER_OFFSET)?,
            consumer: field(ControlRingState::CONSUMER_OFFSET)?,
        })
    }

    const fn io<M>(self, memory: &M) -> MemoryRingIo<'_, M> {
        MemoryRingIo::new(memory, self.producer, self.consumer)
    }
}

/// Checked addresses of the command and response cursors in a control queue record.
#[derive(Clone, Copy, Debug)]
pub struct ControlQueueLayout {
    command: Cursors,
    response: Cursors,
    event: Cursors,
}

impl ControlQueueLayout {
    /// Validates record alignment and address range without accessing memory.
    pub fn new(base: MemoryAddress) -> Result<Self, RingError> {
        let alignment =
            u64::try_from(ControlQueue::RECORD_ALIGNMENT).map_err(|_| RingError::Geometry)?;
        let size = u64::try_from(ControlQueue::RECORD_SIZE).map_err(|_| RingError::Geometry)?;
        if base.raw() % alignment != 0 || base.raw().checked_add(size).is_none() {
            return Err(RingError::Geometry);
        }
        Ok(Self {
            command: Cursors::new(base, ControlQueue::COMMAND_OFFSET)?,
            response: Cursors::new(base, ControlQueue::RESPONSE_OFFSET)?,
            event: Cursors::new(base, ControlQueue::EVENT_OFFSET)?,
        })
    }

    /// Borrows memory at the control command ring's cursor addresses.
    pub const fn command_io<'a, M>(&self, memory: &'a M) -> MemoryRingIo<'a, M> {
        self.command.io(memory)
    }

    /// Borrows the independent event cursor fields.
    pub const fn event_io<'a, M>(&self, memory: &'a M) -> MemoryRingIo<'a, M> {
        self.event.io(memory)
    }

    /// Borrows memory at the control response ring's cursor addresses.
    pub const fn response_io<'a, M>(&self, memory: &'a M) -> MemoryRingIo<'a, M> {
        self.response.io(memory)
    }
}
