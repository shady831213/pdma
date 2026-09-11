mod bus;
mod control;
mod rings;

use crate::{Error, RingConfig};
use bus::RegisterPort;
use hardware_interface_runtime::{CpuOwner, MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    ChannelRegisters, Command, CommandRegisters, DeviceRegisters, Response, ResponseRegisters, Ring,
};
use rings::{CommandRing, ResponseRing};

const COMMAND: usize = DeviceRegisters::COMMAND.offset();
const RESPONSE: usize = DeviceRegisters::RESPONSE.offset();
const CHANNEL: usize = DeviceRegisters::CHANNEL.offset();

/// The command and response storage selected by software.
#[derive(Clone, Copy, Debug)]
pub struct Rings {
    /// Software-produced command records.
    pub command: RingConfig<Command>,
    /// Hardware-produced response records.
    pub response: RingConfig<Response>,
}

/// Sole software ring cursor owner, bound to its setup CPU.
#[derive(Debug)]
pub struct Channel<B, M> {
    port: RegisterPort<B>,
    memory: M,
    command: CommandRing,
    response: ResponseRing,
}

impl<B: RegisterAccess + Send + Sync, M: MemoryAccess + Send + Sync> Channel<B, M> {
    /// Programs disabled hardware and enables channel and device interrupts.
    ///
    /// Cancellation may leave partial setup; issue channel reset through the bus before retrying.
    pub async fn setup(bus: B, memory: M, rings: Rings) -> Result<Self, Error<B::Error>> {
        let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
        let channel = Self {
            port: RegisterPort::new(bus, owner),
            memory,
            command: CommandRing::new(rings.command),
            response: ResponseRing::new(rings.response),
        };
        channel
            .port
            .write(
                CHANNEL + ChannelRegisters::ENABLE.offset(),
                &0_u32.to_le_bytes(),
            )
            .await?;
        channel
            .port
            .write(
                COMMAND + CommandRegisters::BASE.offset(),
                &rings.command.base().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .write(
                COMMAND + CommandRegisters::CAPACITY.offset(),
                &rings.command.capacity().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .write(
                RESPONSE + ResponseRegisters::BASE.offset(),
                &rings.response.base().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .write(
                RESPONSE + ResponseRegisters::CAPACITY.offset(),
                &rings.response.capacity().to_le_bytes(),
            )
            .await?;
        channel.set_irq_enabled(true).await?;
        channel.set_device_irq_enabled(true).await?;
        channel
            .port
            .write(
                CHANNEL + ChannelRegisters::ENABLE.offset(),
                &1_u32.to_le_bytes(),
            )
            .await?;
        Ok(channel)
    }

    /// Writes the descriptor before publishing its producer cursor.
    ///
    /// Cancellation during the final bus write has the transport's commit semantics.
    /// Read hardware cursors before deciding whether to retry that submission.
    pub async fn submit(&mut self, command: Command) -> Result<(), Error<B::Error>> {
        self.command.push(&self.port, &self.memory, command).await
    }

    /// Reads and decodes one response before advancing its consumer cursor.
    ///
    /// Cancellation during the final bus write has the transport's commit semantics.
    pub async fn response(&mut self) -> Result<Option<Response>, Error<B::Error>> {
        self.response.pop(&self.port, &self.memory).await
    }
}
