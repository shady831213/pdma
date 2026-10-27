//! The complete software program and its persistent interrupt handler.

use pdma_model_contract::dma::{
    ChannelFatalCode, ChannelId, Command, CommandHeader, CopyCommand, Event, FenceCommand, Opcode,
    Segment, TrapCommand, WordTarget, WordTargetWidth, WordWrite,
};

mod buffers;

use hardware_interface_irq::InterruptHandler;
use hardware_interface_memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess, SharedMemoryAllocator,
};
use hardware_interface_register::RegisterAccess;
use hardware_interface_runtime::InterruptRegistration;
use pdma_model_host::{
    Channel, ChannelConfig, DmaError as HostError, RingConfig, Rings, runtime::Irq,
};
use std::error::Error;
use tokio::sync::oneshot;

/// Software-selected representation of one payload stream.
#[derive(Clone, Copy, Debug)]
pub enum BufferLayout<'a> {
    /// One contiguous allocation.
    Linear,
    /// Independently allocated spans with these byte lengths.
    ScatterGather(&'a [usize]),
}

/// Payload and independent source/destination allocation layouts.
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    /// Bytes initialized by software before submission.
    pub data: &'a [u8],
    /// Source representation.
    pub source: BufferLayout<'a>,
    /// Destination representation.
    pub destination: BufferLayout<'a>,
}

impl<'a> Input<'a> {
    /// Uses contiguous buffers for both endpoints.
    pub const fn linear(data: &'a [u8]) -> Self {
        Self {
            data,
            source: BufferLayout::Linear,
            destination: BufferLayout::Linear,
        }
    }
}

/// Result delivered from the ISR to the main software program.
pub type Completion = (Option<Event>, ChannelFatalCode, bool);
/// Platform capabilities visible to the software program.
#[derive(Debug)]
pub struct Ports<'a, B> {
    /// Register bus, without access to device internals.
    pub bus: B,
    /// Software memory access supplied by the platform.
    pub memory: SharedMemoryAccess,
    /// Allocation service over the platform's backing memory.
    pub allocator: SharedMemoryAllocator,
    /// CPU-bound ISR registration supplied by the runtime.
    pub interrupts: InterruptRegistration<'a, Irq, CompletionHandler<B>>,
}

/// Software completion with the destination available for independent verification.
#[derive(Debug)]
pub struct Outcome {
    /// Event consumed by the ISR, if hardware published one.
    pub event: Option<Event>,
    /// Sticky fatal observed by the ISR.
    pub fatal: ChannelFatalCode,
    #[doc = "Whether the ISR observed the software-selected Fence value."]
    pub completed: bool,
    /// Software-allocated destination of the DMA copy.
    pub destination: Vec<Segment>,
}

/// Persistent software owner of event consumption and IRQ masking.
#[derive(Debug)]
pub struct CompletionHandler<B> {
    channel: Channel<B, SharedMemoryAccess>,
    completion: Option<oneshot::Sender<Completion>>,
    memory: SharedMemoryAccess,
    fence: MemoryAddress,
}

impl<B: RegisterAccess + Send + Sync> InterruptHandler<Irq> for CompletionHandler<B> {
    type Error = HostError<B::Error>;

    async fn handle(&mut self, _: &Irq) -> Result<(), Self::Error> {
        let fatal = self.channel.fatal().await?;
        let event = self.channel.event().await?;
        match fatal {
            ChannelFatalCode::None => {}
            ChannelFatalCode::CommandRead
            | ChannelFatalCode::EventWrite
            | ChannelFatalCode::EventFull
            | ChannelFatalCode::EventConfiguration => {
                self.channel.set_irq_enabled(false).await?;
            }
        }
        self.channel.acknowledge_trap().await?;
        let mut bytes = [0; 8];
        self.memory
            .read(
                self.fence,
                MemoryAlignment::try_new(8).map_err(|_| HostError::Geometry)?,
                &mut bytes,
            )
            .map_err(HostError::Memory)?;
        let completed = u64::from_le_bytes(bytes) == 7;
        if let Some(completion) = self.completion.take() {
            let _receiver_closed = completion.send((event, fatal, completed));
        }
        Ok(())
    }
}

/// Run the complete software program on the caller's active software CPU.
pub async fn main<B>(
    ports: Ports<'_, B>,
    input: Input<'_>,
) -> Result<Outcome, Box<dyn Error + Send + Sync>>
where
    B: RegisterAccess + Send + Sync,
    B::Error: Error + Send + Sync + 'static,
{
    const CAPACITY: usize = 4;
    let command = ports.allocator.allocate(
        u64::try_from(CAPACITY * Command::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
    )?;
    let event = ports.allocator.allocate(
        u64::try_from(CAPACITY * Event::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
    )?;
    let buffers = buffers::Builder {
        allocator: &ports.allocator,
        memory: &ports.memory,
    };
    let source = buffers.build(input.source, input.data)?;
    let destination = buffers.build(input.destination, &vec![0; input.data.len()])?;
    let rings = Rings {
        command: RingConfig::new(command.raw(), u32::try_from(CAPACITY)?)?,
        event: RingConfig::new(event.raw(), u32::try_from(CAPACITY)?)?,
    };
    let config = ChannelConfig {
        id: ChannelId::new(0),
        rings,
    };
    let fence = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
    ports
        .memory
        .write(fence, MemoryAlignment::try_new(8)?, &0_u64.to_le_bytes())?;
    let mut channel = Channel::setup(ports.bus, ports.memory.clone(), config).await?;
    channel
        .submit(Command::Copy(CopyCommand {
            id: 7,
            copy_length: u64::try_from(input.data.len())?,
            sg_context: 0,
            source: source.endpoint,
            destination: match destination.endpoint {
                pdma_model_contract::dma::CopyCommandSourceValues::Linear(endpoint) => {
                    pdma_model_contract::dma::CopyCommandDestinationValues::Linear(endpoint)
                }
                pdma_model_contract::dma::CopyCommandSourceValues::ScatterGather(endpoint) => {
                    pdma_model_contract::dma::CopyCommandDestinationValues::ScatterGather(endpoint)
                }
            },
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }))
        .await?;
    channel
        .submit(Command::Fence(FenceCommand {
            header: CommandHeader {
                opcode: Opcode::Fence,
            },
            id: 8,
            word: WordWrite {
                target: WordTarget {
                    address: fence.raw(),
                    context: 0,
                    width: WordTargetWidth::Bits64,
                },
                value: 7,
            },
        }))
        .await?;
    channel
        .submit(Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 9,
        }))
        .await?;

    let (completion, received) = oneshot::channel();
    ports.interrupts.register(
        (),
        CompletionHandler {
            channel,
            completion: Some(completion),
            memory: ports.memory,
            fence,
        },
    )?;
    let (event, fatal, completed) = received.await?;
    Ok(Outcome {
        event,
        fatal,
        completed,
        destination: destination.segments,
    })
}
