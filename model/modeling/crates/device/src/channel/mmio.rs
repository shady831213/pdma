use super::{Channel, CommandRing, ResponseRing};
use crate::Error;
use hardware_interface_schema::{Access, RegisterMapLayout};
use pdma_model_contract::{
    ChannelRegisters, CommandRegisters, DeviceRegisters, ResponseRegisters, Ring,
};

impl Channel {
    pub(crate) fn read(&mut self, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        let result = if offset < DeviceRegisters::RESPONSE.offset() {
            read::<CommandRegisters>(&self.command.registers, offset, out)
        } else if offset < DeviceRegisters::CHANNEL.offset() {
            read::<ResponseRegisters>(
                &self.response.registers,
                offset - DeviceRegisters::RESPONSE.offset(),
                out,
            )
        } else {
            read::<ChannelRegisters>(
                &self.registers,
                offset - DeviceRegisters::CHANNEL.offset(),
                out,
            )
        };
        self.update_irq();
        result
    }

    pub(crate) fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        if offset < DeviceRegisters::RESPONSE.offset() {
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
        } else if offset < DeviceRegisters::CHANNEL.offset() {
            let mut next = self.response.registers;
            write::<ResponseRegisters>(
                &mut next,
                offset - DeviceRegisters::RESPONSE.offset(),
                bytes,
            )?;
            if self.configured
                && (next.base != self.response.registers.base
                    || next.capacity != self.response.registers.capacity)
            {
                return Err(Error::ConfigurationLocked);
            }
            let candidate = ResponseRing { registers: next };
            candidate
                .with_consumer(self.response.pointers(), next.consumer.raw())
                .map_err(|_| Error::Cursor)?;
            self.response = candidate;
        } else {
            let mut next = self.registers;
            write::<ChannelRegisters>(
                &mut next,
                offset - DeviceRegisters::CHANNEL.offset(),
                bytes,
            )?;
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

fn read<M: RegisterMapLayout>(
    values: &M::Value,
    offset: usize,
    out: &mut [u8],
) -> Result<(), Error> {
    let definition = M::register_definition(offset).ok_or(Error::Unmapped)?;
    if out.len() != definition.width() {
        return Err(Error::Width);
    }
    if definition.access() == Access::WriteOnly {
        return Err(Error::Access);
    }
    let raw = M::read_register(values, offset).ok_or(Error::Unmapped)?;
    definition
        .view_mut(out)
        .and_then(|mut view| view.set_value(raw))
        .map_err(|_| Error::Width)
}

fn write<M: RegisterMapLayout>(
    values: &mut M::Value,
    offset: usize,
    bytes: &[u8],
) -> Result<(), Error> {
    let definition = M::register_definition(offset).ok_or(Error::Unmapped)?;
    if bytes.len() != definition.width() {
        return Err(Error::Width);
    }
    if definition.access() == Access::ReadOnly {
        return Err(Error::Access);
    }
    let raw = definition
        .view(bytes)
        .and_then(|view| view.value())
        .map_err(|_| Error::Width)?;
    let old = M::read_register(values, offset).ok_or(Error::Unmapped)?;
    if (raw ^ old) & !definition.writable_mask() != 0 {
        return Err(Error::ReservedBits);
    }
    M::write_register(values, offset, raw).ok_or(Error::Width)
}
