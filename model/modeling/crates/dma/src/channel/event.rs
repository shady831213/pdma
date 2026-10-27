use crate::{ChannelFatal, Error};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_schema::{Encode, RegisterAccessError, RegisterWindow};
use pdma_model_contract::dma::{Event, EventRegisters};
use pdma_model_contract::register::ReadWordValues;
use pdma_model_contract::ring::{Ring, RingPointers};

/// Event queue state, including its own generated registers.
#[derive(Debug)]
pub struct EventRing {
    pub(crate) registers: RegisterWindow<EventRegisters>,
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Ring for EventRing {
    type Entry = Event;

    fn base(&self) -> u64 {
        self.registers.values.base.raw()
    }

    fn capacity(&self) -> u32 {
        self.registers.values.capacity.raw()
    }
}

impl EventRing {
    /// Places this ring's MMIO window in its parent's address space.
    pub const fn new(base: usize) -> Self {
        Self {
            registers: RegisterWindow::new(base),
        }
    }

    pub(crate) const fn pointers(&self) -> RingPointers {
        RingPointers {
            producer: self.registers.values.producer.raw(),
            consumer: self.registers.values.consumer.raw(),
        }
    }

    pub(crate) fn read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        self.registers
            .read(address, out)
            .map_err(RegisterAccessError::widen)
    }

    pub(crate) fn write(
        &mut self,
        address: usize,
        bytes: &[u8],
        configured: bool,
        resetting: bool,
    ) -> Result<(), RegisterAccessError<Error>> {
        let pointers = self.pointers();
        let base = self.registers.base();
        self.registers.write_with(
            address,
            bytes,
            |_, old, next| {
                if resetting {
                    return Err(Error::ResetInProgress);
                }
                if configured && (next.base != old.base || next.capacity != old.capacity) {
                    return Err(Error::ConfigurationLocked);
                }
                let mut candidate = Self::new(base);
                candidate.registers.values = *next;
                candidate
                    .with_consumer(pointers, next.consumer.raw())
                    .map_err(|_| Error::Cursor)?;
                Ok(())
            },
            |_, _| {},
        )
    }

    pub(crate) fn push(
        &mut self,
        memory: &impl MemoryAccess,
        event: &Event,
    ) -> Result<(), ChannelFatal> {
        let pointers = self.pointers();
        let address = MemoryAddress::new(
            self.entry(pointers.producer)
                .map_err(|_| ChannelFatal::EventConfiguration)?,
        );
        if pointers.pending() == self.capacity() {
            return Err(ChannelFatal::EventFull);
        }
        let next = self
            .advance_producer(pointers, 1)
            .map_err(|_| ChannelFatal::EventConfiguration)?;
        let raw = event.encode().map_err(|_| ChannelFatal::EventWrite)?;
        memory
            .write_buffer(address, MemoryAlignment::ONE, &raw)
            .map_err(|_| ChannelFatal::EventWrite)?;
        self.registers.values.producer = ReadWordValues::new(next.producer);
        Ok(())
    }
}
