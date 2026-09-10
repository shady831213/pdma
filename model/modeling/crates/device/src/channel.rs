mod mmio;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{
    ChannelFaultCode, ChannelRegistersValues, Command, CommandRaw, CommandRegistersValues,
    ReadWordValues, Response, ResponseRegistersValues, Ring, RingPointers,
};

use crate::Fault;

/// Command queue state, including its own generated registers.
#[derive(Debug, Default)]
pub struct CommandRing {
    pub(crate) registers: CommandRegistersValues,
}

impl Ring for CommandRing {
    type Entry = Command;

    fn base(&self) -> u64 {
        self.registers.base.raw()
    }

    fn capacity(&self) -> u32 {
        self.registers.capacity.raw()
    }
}

impl CommandRing {
    pub(crate) const fn pointers(&self) -> RingPointers {
        RingPointers {
            producer: self.registers.producer.raw(),
            consumer: self.registers.consumer.raw(),
        }
    }

    pub(crate) fn pop(&mut self, memory: &impl MemoryAccess) -> Result<Option<Command>, Fault> {
        let pointers = self.pointers();
        if pointers.is_empty() {
            return Ok(None);
        }
        let address = MemoryAddress::new(
            self.entry(pointers.consumer)
                .map_err(|_| Fault::CommandRead)?,
        );
        let next = self
            .advance_consumer(pointers, 1)
            .map_err(|_| Fault::CommandRead)?;
        let raw = memory
            .read_typed::<CommandRaw>(address, MemoryAlignment::ONE)
            .map_err(|_| Fault::CommandRead)?;
        let command = raw.decode().map_err(|_| Fault::CommandRead)?;
        self.registers.consumer = ReadWordValues::new(next.consumer);
        Ok(Some(command))
    }
}

/// Response queue state, including its own generated registers.
#[derive(Debug, Default)]
pub struct ResponseRing {
    pub(crate) registers: ResponseRegistersValues,
}

impl Ring for ResponseRing {
    type Entry = Response;

    fn base(&self) -> u64 {
        self.registers.base.raw()
    }

    fn capacity(&self) -> u32 {
        self.registers.capacity.raw()
    }
}

impl ResponseRing {
    pub(crate) const fn pointers(&self) -> RingPointers {
        RingPointers {
            producer: self.registers.producer.raw(),
            consumer: self.registers.consumer.raw(),
        }
    }

    pub(crate) fn push(
        &mut self,
        memory: &impl MemoryAccess,
        response: &Response,
    ) -> Result<bool, Fault> {
        let pointers = self.pointers();
        let address = MemoryAddress::new(
            self.entry(pointers.producer)
                .map_err(|_| Fault::ResponseWrite)?,
        );
        if pointers.pending() == self.capacity() {
            return Ok(false);
        }
        let next = self
            .advance_producer(pointers, 1)
            .map_err(|_| Fault::ResponseWrite)?;
        let raw = response.encode().map_err(|_| Fault::ResponseWrite)?;
        memory
            .write_typed(address, MemoryAlignment::ONE, &raw)
            .map_err(|_| Fault::ResponseWrite)?;
        self.registers.producer = ReadWordValues::new(next.producer);
        Ok(true)
    }
}

/// One channel owns two rings, sticky fault state and interrupt controls.
#[derive(Debug, Default)]
pub struct Channel {
    /// Host-produced command ring.
    pub command: CommandRing,
    /// Device-produced response ring.
    pub response: ResponseRing,
    pub(crate) registers: ChannelRegistersValues,
    pub(crate) configured: bool,
    pub(crate) irq: bool,
}

impl Channel {
    pub(crate) const fn latch(&mut self, fault: Fault) {
        self.registers.fault = self.registers.fault.with_code(fault.code());
    }

    pub(crate) fn update_irq(&mut self) {
        self.irq = self.registers.irq_enable.raw() != 0
            && (self.halted() || !self.response.pointers().is_empty());
    }

    pub(crate) fn halted(&self) -> bool {
        match self.registers.fault.code() {
            Ok(ChannelFaultCode::None) => false,
            Ok(ChannelFaultCode::CommandRead | ChannelFaultCode::ResponseWrite) | Err(_) => true,
        }
    }
    pub(crate) const fn enabled(&self) -> bool {
        self.registers.enable.raw() != 0
    }
}
