use super::{COMMAND, RESPONSE, bus::RegisterPort};
use crate::{Error, RingConfig};
use hardware_interface_runtime::{CpuContextError, MemoryAccess, RegisterAccess};
use pdma_model_contract::{Command, CommandRegisters, Response, ResponseRegisters};
use pdma_model_software::ring::{RingCursorRegisters, SoftRing};

#[derive(Debug)]
pub(super) struct CommandRing(SoftRing<Command>);

impl CommandRing {
    pub(super) fn new(config: RingConfig<Command>) -> Result<Self, CpuContextError> {
        SoftRing::new(config).map(Self)
    }

    pub(super) const fn has_pending(&self) -> bool {
        self.0.has_pending_push()
    }

    pub(super) async fn push<B, M>(
        &mut self,
        port: &RegisterPort<B>,
        memory: &M,
        command: Command,
    ) -> Result<(), Error<B::Error>>
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
    ) -> Result<(), Error<B::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        let io = port.ring_io(memory, command_registers()?)?;
        self.0.resume_push(&io).await.map_err(Error::from)
    }
}

fn command_registers<B>() -> Result<RingCursorRegisters, Error<B>> {
    Ok(RingCursorRegisters {
        producer: u32::try_from(COMMAND + CommandRegisters::PRODUCER.offset())
            .map_err(|_| Error::Geometry)?,
        consumer: u32::try_from(COMMAND + CommandRegisters::CONSUMER.offset())
            .map_err(|_| Error::Geometry)?,
    })
}

#[derive(Debug)]
pub(super) struct ResponseRing(SoftRing<Response>);

impl ResponseRing {
    pub(super) fn new(config: RingConfig<Response>) -> Result<Self, CpuContextError> {
        SoftRing::new(config).map(Self)
    }

    pub(super) async fn pop<B, M>(
        &mut self,
        port: &RegisterPort<B>,
        memory: &M,
    ) -> Result<Option<Response>, Error<B::Error>>
    where
        B: RegisterAccess + Send + Sync,
        M: MemoryAccess + Send + Sync,
    {
        let registers = RingCursorRegisters {
            producer: u32::try_from(RESPONSE + ResponseRegisters::PRODUCER.offset())
                .map_err(|_| Error::Geometry)?,
            consumer: u32::try_from(RESPONSE + ResponseRegisters::CONSUMER.offset())
                .map_err(|_| Error::Geometry)?,
        };
        let io = port.ring_io(memory, registers)?;
        let Some(response) = self.0.peek(&io).await? else {
            return Ok(None);
        };
        self.0.consume(&io).await?;
        Ok(Some(response))
    }
}
