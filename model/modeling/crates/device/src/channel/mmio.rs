use super::{Channel, CommandRing, ResponseRing};
use crate::{Error, read_register as read, write_register as write};
use pdma_model_contract::{
    ChannelBlock, ChannelRegisters, CommandRegisters, ResponseRegisters, Ring,
};

impl Channel {
    pub(crate) fn read(&mut self, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        let result = if offset < ChannelBlock::RESPONSE.offset() {
            read::<CommandRegisters>(&self.command.registers, offset, out)
        } else if offset < ChannelBlock::CONTROL.offset() {
            read::<ResponseRegisters>(
                &self.response.registers,
                offset - ChannelBlock::RESPONSE.offset(),
                out,
            )
        } else {
            read::<ChannelRegisters>(
                &self.registers,
                offset - ChannelBlock::CONTROL.offset(),
                out,
            )
        };
        self.update_irq();
        result
    }

    pub(crate) fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        if offset < ChannelBlock::RESPONSE.offset() {
            let mut next = self.command.registers;
            write::<CommandRegisters>(&mut next, offset, bytes)?;
            if self.configured
                && (next.base != self.command.registers.base
                    || next.capacity != self.command.registers.capacity)
            {
                return Err(Error::ConfigurationLocked);
            }
            let candidate = CommandRing { registers: next };
            candidate
                .with_producer(self.command.pointers(), next.producer.raw())
                .map_err(|_| Error::Cursor)?;
            self.command = candidate;
        } else if offset < ChannelBlock::CONTROL.offset() {
            let mut next = self.response.registers;
            write::<ResponseRegisters>(&mut next, offset - ChannelBlock::RESPONSE.offset(), bytes)?;
            if self.configured
                && (next.base != self.response.registers.base
                    || next.capacity != self.response.registers.capacity)
            {
                return Err(Error::ConfigurationLocked);
            }
            let mut candidate = ResponseRing {
                registers: next,
                last_ioc: self.response.last_ioc,
            };
            candidate
                .with_consumer(self.response.pointers(), next.consumer.raw())
                .map_err(|_| Error::Cursor)?;
            if candidate.last_ioc.is_some_and(|boundary| {
                boundary.wrapping_sub(self.response.pointers().consumer)
                    <= next
                        .consumer
                        .raw()
                        .wrapping_sub(self.response.pointers().consumer)
            }) {
                candidate.last_ioc = None;
            }
            self.response = candidate;
        } else {
            let mut next = self.registers;
            write::<ChannelRegisters>(&mut next, offset - ChannelBlock::CONTROL.offset(), bytes)?;
            if next.reset.raw() != 0 {
                self.reset();
                return Ok(());
            }
            if next.enable.raw() != 0 {
                self.validate_geometry()?;
                self.configured = true;
            }
            self.registers = next;
        }
        self.update_irq();
        Ok(())
    }

    fn validate_geometry(&self) -> Result<(), Error> {
        self.command
            .validate_geometry()
            .map_err(|_| Error::Configuration)?;
        self.response
            .validate_geometry()
            .map_err(|_| Error::Configuration)?;
        Ok(())
    }
}
