//! PDMA channel driver and register-only control.

mod bus;
mod context;
mod control;
mod driver_control;
mod error;
pub use control::ChannelControl;
pub use error::Error;
mod registers;
mod rings;

use crate::ring::RingConfig;
use bus::RegisterPort;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Response,
    ResponseRegisters,
};
use pdma_model_contract::ring::Ring;
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

/// Sole software ring cursor owner.
#[derive(Debug)]
pub struct Channel<B, M> {
    port: RegisterPort<B>,
    memory: M,
    command: CommandRing,
    response: ResponseRing,
}

impl<B: RegisterAccess + Send + Sync, M: Send + Sync> Channel<B, M> {
    /// Discovers and programs one channel, enabling its interrupt gate.
    ///
    /// Cancellation may leave partial setup; request channel reset through the bus and
    /// wait for its reset register to read zero before retrying.
    pub async fn setup(bus: B, memory: M, config: ChannelConfig) -> Result<Self, Error<B::Error>> {
        Self::setup_at(bus, memory, config, 0).await
    }

    /// Programs a channel in a DMA block placed in the caller's register bus.
    pub async fn setup_at(
        bus: B,
        memory: M,
        config: ChannelConfig,
        dma_base: u32,
    ) -> Result<Self, Error<B::Error>> {
        let ChannelConfig { id, rings } = config;
        let channel = Self {
            port: RegisterPort::new(bus, id, dma_base).await?,
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
}

impl<B: RegisterAccess + Send + Sync, M: MemoryAccess + Send + Sync> Channel<B, M> {
    /// Writes the descriptor before publishing its producer cursor.
    ///
    /// Cancellation retains an accepted command for `resume_submission`.
    /// A new submission is rejected while that publication remains pending.
    pub async fn submit(&mut self, command: Command) -> Result<(), Error<B::Error, M::Error>> {
        self.command.push(&self.port, &self.memory, command).await
    }

    /// Whether an accepted command still needs publication reconciliation.
    pub const fn has_pending_submission(&self) -> bool {
        self.command.has_pending()
    }

    /// Completes an accepted submission without repeating a committed publication.
    pub async fn resume_submission(&mut self) -> Result<(), Error<B::Error, M::Error>> {
        self.command.resume(&self.port, &self.memory).await
    }

    /// Reads and decodes one response before advancing its consumer cursor.
    ///
    /// Cancellation retains the decoded response; another call reconciles consumption.
    pub async fn response(&mut self) -> Result<Option<Response>, Error<B::Error, M::Error>> {
        self.response.pop(&self.port, &self.memory).await
    }
}
