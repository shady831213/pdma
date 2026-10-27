use super::Error;
use super::{COMMAND, EVENT, bus::RegisterPort};
use crate::ring::RingConfig;
use crate::ring::{RingCursorRegisters, SoftRing};
use hardware_interface_io::memory::MemoryAccess;
use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::dma::{Command, CommandRegisters, Event, EventRegisters};

#[derive(Debug)]
pub(super) struct CommandRing(SoftRing<Command>);

impl CommandRing {
    pub(super) const fn new(config: RingConfig<Command>) -> Self {
        Self(SoftRing::new(config))
    }

    pub(super) const fn has_pending(&self) -> bool {
        self.0.has_pending_push()
    }

    pub(super) async fn push<B, M>(
        &mut self,
        port: &RegisterPort<B>,
        memory: &M,
        command: Command,
    ) -> Result<(), Error<B::Error, M::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        let io = port.ring_io(memory, command_registers()?)?;
        if self.0.try_push(&io, command).await? {
            Ok(())
        } else {
            Err(Error::Full)
        }
    }

    pub(super) async fn resume<B, M>(
        &mut self,
        port: &RegisterPort<B>,
        memory: &M,
    ) -> Result<(), Error<B::Error, M::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        let io = port.ring_io(memory, command_registers()?)?;
        self.0.resume_push(&io).await.map_err(Error::from)
    }
}

fn command_registers<B, E>() -> Result<RingCursorRegisters, Error<B, E>> {
    Ok(RingCursorRegisters {
        producer: u32::try_from(COMMAND + CommandRegisters::PRODUCER.offset())
            .map_err(|_| Error::Geometry)?,
        consumer: u32::try_from(COMMAND + CommandRegisters::CONSUMER.offset())
            .map_err(|_| Error::Geometry)?,
    })
}

#[derive(Debug)]
pub(super) struct EventRing(SoftRing<Event>);

impl EventRing {
    pub(super) const fn new(config: RingConfig<Event>) -> Self {
        Self(SoftRing::new(config))
    }

    pub(super) async fn pop<B, M>(
        &mut self,
        port: &RegisterPort<B>,
        memory: &M,
    ) -> Result<Option<Event>, Error<B::Error, M::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        let registers = RingCursorRegisters {
            producer: u32::try_from(EVENT + EventRegisters::PRODUCER.offset())
                .map_err(|_| Error::Geometry)?,
            consumer: u32::try_from(EVENT + EventRegisters::CONSUMER.offset())
                .map_err(|_| Error::Geometry)?,
        };
        let io = port.ring_io(memory, registers)?;
        let Some(event) = self.0.peek(&io).await? else {
            return Ok(None);
        };
        self.0.consume(&io).await?;
        Ok(Some(event))
    }
}
