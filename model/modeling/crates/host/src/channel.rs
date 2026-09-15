mod bus;
mod control;
mod rings;

use crate::{Error, RingConfig};
use bus::RegisterPort;
use hardware_interface_runtime::{CpuOwner, MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Response,
    ResponseRegisters, Ring,
};
use rings::{CommandRing, ResponseRing};

const COMMAND: usize = ChannelBlock::COMMAND.offset();
const RESPONSE: usize = ChannelBlock::RESPONSE.offset();
const CHANNEL: usize = ChannelBlock::CONTROL.offset();

/// The command and response storage selected by software.
#[derive(Clone, Copy, Debug)]
pub struct Rings {
    /// Software-produced command records.
    pub command: RingConfig<Command>,
    /// Hardware-produced response records.
    pub response: RingConfig<Response>,
}

/// A hardware channel and its software-owned ring storage.
#[derive(Clone, Copy, Debug)]
pub struct ChannelConfig {
    /// Channel selected from hardware discovery.
    pub id: ChannelId,
    /// Storage allocated by the software caller.
    pub rings: Rings,
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
    /// Discovers and programs one channel, enabling its interrupt gate.
    ///
    /// Cancellation may leave partial setup; request channel reset through the bus and
    /// wait for its reset register to read zero before retrying.
    pub async fn setup(bus: B, memory: M, config: ChannelConfig) -> Result<Self, Error<B::Error>> {
        let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
        let ChannelConfig { id, rings } = config;
        let channel = Self {
            port: RegisterPort::new(bus, owner, id).await?,
            memory,
            command: CommandRing::new(rings.command),
            response: ResponseRing::new(rings.response),
        };
        channel
            .port
            .channel_write(
                CHANNEL + ChannelRegisters::ENABLE.offset(),
                &0_u32.to_le_bytes(),
            )
            .await?;
        channel
            .port
            .channel_write(
                COMMAND + CommandRegisters::BASE.offset(),
                &rings.command.base().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .channel_write(
                COMMAND + CommandRegisters::CAPACITY.offset(),
                &rings.command.capacity().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .channel_write(
                RESPONSE + ResponseRegisters::BASE.offset(),
                &rings.response.base().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .channel_write(
                RESPONSE + ResponseRegisters::CAPACITY.offset(),
                &rings.response.capacity().to_le_bytes(),
            )
            .await?;
        channel.set_irq_enabled(true).await?;
        channel
            .port
            .channel_write(
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
