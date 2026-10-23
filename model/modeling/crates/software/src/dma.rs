//! PDMA channel driver and register-only control.

use pdma_model_contract::dma::ChannelConfiguration;
mod bus;
mod completion;
mod context;
pub use completion::CompletionWord;
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
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Event, EventRegisters,
};
use pdma_model_contract::ring::Ring;
use rings::{CommandRing, EventRing};

const COMMAND: usize = ChannelBlock::COMMAND.offset();
const EVENT: usize = ChannelBlock::EVENT.offset();
const CHANNEL: usize = ChannelBlock::CONTROL.offset();

/// The command and event storage selected by software.
#[derive(Clone, Copy, Debug)]
pub struct Rings {
    /// Software-produced command records.
    pub command: RingConfig<Command>,
    /// Hardware-produced event records.
    pub event: RingConfig<Event>,
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
    event: EventRing,
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
            event: EventRing::new(rings.event),
        };
        channel
            .port
            .channel_write(
                CHANNEL + ChannelRegisters::CONFIG.offset() + ChannelConfiguration::ENABLE.offset(),
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
                EVENT + EventRegisters::BASE.offset(),
                &rings.event.base().to_le_bytes(),
            )
            .await?;
        channel
            .port
            .channel_write(
                EVENT + EventRegisters::CAPACITY.offset(),
                &rings.event.capacity().to_le_bytes(),
            )
            .await?;
        channel.set_irq_enabled(true).await?;
        channel
            .port
            .channel_write(
                CHANNEL + ChannelRegisters::CONFIG.offset() + ChannelConfiguration::ENABLE.offset(),
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

    /// Reads and decodes one event before advancing its consumer cursor.
    ///
    /// Cancellation retains the decoded event; another call reconciles consumption.
    pub async fn event(&mut self) -> Result<Option<Event>, Error<B::Error, M::Error>> {
        self.event.pop(&self.port, &self.memory).await
    }
}
