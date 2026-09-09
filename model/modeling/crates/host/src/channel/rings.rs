use super::{COMMAND, RESPONSE, bus::RegisterPort};
use crate::{Error, RingConfig};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{
    Command, CommandRegisters, Response, ResponseRaw, ResponseRegisters, Ring, RingPointers,
};

#[derive(Debug)]
pub(super) struct CommandRing {
    config: RingConfig<Command>,
}

impl Ring for CommandRing {
    type Entry = Command;

    fn base(&self) -> u64 {
        self.config.base()
    }

    fn capacity(&self) -> u32 {
        self.config.capacity()
    }
}

impl CommandRing {
    pub(super) const fn new(config: RingConfig<Command>) -> Self {
        Self { config }
    }

    pub(super) async fn push<B, M>(
        &self,
        port: &RegisterPort<B>,
        memory: &M,
        command: Command,
    ) -> Result<(), Error<B::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        port.validate_active()?;
        let producer = port
            .read_word(COMMAND + CommandRegisters::PRODUCER.offset())
            .await?;
        let consumer = port
            .read_word(COMMAND + CommandRegisters::CONSUMER.offset())
            .await?;
        let pointers = RingPointers { producer, consumer };
        if pointers.pending() >= self.capacity() {
            return Err(Error::Full);
        }
        let next = self.advance_producer(pointers, 1).map_err(Error::Ring)?;
        let address = MemoryAddress::new(self.entry(producer).map_err(Error::Ring)?);
        memory
            .write_typed(
                address,
                MemoryAlignment::ONE,
                &command.encode().map_err(Error::Codec)?,
            )
            .map_err(Error::Memory)?;
        port.write(
            COMMAND + CommandRegisters::PRODUCER.offset(),
            &next.producer.to_le_bytes(),
        )
        .await
    }
}

#[derive(Debug)]
pub(super) struct ResponseRing {
    config: RingConfig<Response>,
}

impl Ring for ResponseRing {
    type Entry = Response;

    fn base(&self) -> u64 {
        self.config.base()
    }

    fn capacity(&self) -> u32 {
        self.config.capacity()
    }
}

impl ResponseRing {
    pub(super) const fn new(config: RingConfig<Response>) -> Self {
        Self { config }
    }

    pub(super) async fn pop<B, M>(
        &self,
        port: &RegisterPort<B>,
        memory: &M,
    ) -> Result<Option<Response>, Error<B::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        port.validate_active()?;
        let consumer = port
            .read_word(RESPONSE + ResponseRegisters::CONSUMER.offset())
            .await?;
        let producer = port
            .read_word(RESPONSE + ResponseRegisters::PRODUCER.offset())
            .await?;
        let pointers = RingPointers { producer, consumer };
        if pointers.is_empty() {
            return Ok(None);
        }
        let next = self.advance_consumer(pointers, 1).map_err(Error::Ring)?;
        let address = MemoryAddress::new(self.entry(consumer).map_err(Error::Ring)?);
        let raw = memory
            .read_typed::<ResponseRaw>(address, MemoryAlignment::ONE)
            .map_err(Error::Memory)?;
        let response = raw.decode().map_err(Error::Codec)?;
        port.write(
            RESPONSE + ResponseRegisters::CONSUMER.offset(),
            &next.consumer.to_le_bytes(),
        )
        .await?;
        Ok(Some(response))
    }
}
