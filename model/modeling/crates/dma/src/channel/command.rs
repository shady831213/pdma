use crate::{ChannelFatal, Error};
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_schema::{Decode, RegisterAccessError, RegisterWindow};
use pdma_model_contract::dma::{Command, CommandRaw, CommandRegisters};
use pdma_model_contract::register::ReadWordValues;
use pdma_model_contract::ring::{Ring, RingPointers};

/// Command queue state, including its own generated registers.
#[derive(Debug)]
pub struct CommandRing {
    pub(crate) registers: RegisterWindow<CommandRegisters>,
}

impl Default for CommandRing {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Ring for CommandRing {
    type Entry = Command;

    fn base(&self) -> u64 {
        self.registers.values.base.raw()
    }

    fn capacity(&self) -> u32 {
        self.registers.values.capacity.raw()
    }
}

impl CommandRing {
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
                    .with_producer(pointers, next.producer.raw())
                    .map_err(|_| Error::Cursor)?;
                Ok(())
            },
            |_, _| {},
        )
    }

    pub(crate) fn peek(&self, memory: &impl MemoryAccess) -> Result<Option<Command>, ChannelFatal> {
        let pointers = self.pointers();
        if pointers.is_empty() {
            return Ok(None);
        }
        let address = MemoryAddress::new(
            self.entry(pointers.consumer)
                .map_err(|_| ChannelFatal::CommandRead)?,
        );
        let raw = memory
            .read_buffer::<CommandRaw>(address, MemoryAlignment::ONE)
            .map_err(|_| ChannelFatal::CommandRead)?;
        let command = raw.decode().map_err(|_| ChannelFatal::CommandRead)?;
        Ok(Some(command))
    }

    pub(crate) fn consume(&mut self) -> Result<(), ChannelFatal> {
        let next = self
            .advance_consumer(self.pointers(), 1)
            .map_err(|_| ChannelFatal::CommandRead)?;
        self.registers.values.consumer = ReadWordValues::new(next.consumer);
        Ok(())
    }
}
