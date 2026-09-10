//! The complete software program and its persistent interrupt handler.

mod buffers;

use hardware_interface_runtime::{
    InterruptHandler, InterruptRegistration, MemoryAlignment, RegisterAccess, SharedMemoryAccess,
    SharedMemoryAllocator,
};
use pdma_model_contract::{ChannelFaultCode, Command, Response, Segment};
use pdma_model_host::{Channel, Error as HostError, RingConfig, Rings, runtime::Irq};
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
pub type Completion = (Option<Response>, ChannelFaultCode);
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
    /// Response consumed by the ISR, if hardware published one.
    pub response: Option<Response>,
    /// Sticky fault observed by the ISR.
    pub fault: ChannelFaultCode,
    /// Software-allocated destination of the DMA copy.
    pub destination: Vec<Segment>,
}

/// Persistent software owner of response consumption and IRQ masking.
#[derive(Debug)]
pub struct CompletionHandler<B> {
    channel: Channel<B, SharedMemoryAccess>,
    completion: Option<oneshot::Sender<Completion>>,
}

impl<B: RegisterAccess + Send + Sync> InterruptHandler<Irq> for CompletionHandler<B> {
    type Error = HostError<B::Error>;

    async fn handle(&mut self, _: &Irq) -> Result<(), Self::Error> {
        let fault = self.channel.fault().await?;
        let response = self.channel.response().await?;
        match fault {
            ChannelFaultCode::None => {}
            ChannelFaultCode::CommandRead | ChannelFaultCode::ResponseWrite => {
                self.channel.set_irq_enabled(false).await?;
            }
        }
        if let Some(completion) = self.completion.take() {
            let _receiver_closed = completion.send((response, fault));
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
    const CAPACITY: usize = 2;
    let command = ports.allocator.allocate(
        u64::try_from(CAPACITY * Command::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
    )?;
    let response = ports.allocator.allocate(
        u64::try_from(CAPACITY * Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
    )?;
    let buffers = buffers::Builder {
        allocator: &ports.allocator,
        memory: &ports.memory,
    };
    let source = buffers.build(input.source, input.data)?;
    let destination = buffers.build(input.destination, &vec![0; input.data.len()])?;
    let rings = Rings {
        command: RingConfig::new(command.raw(), u32::try_from(CAPACITY)?)?,
        response: RingConfig::new(response.raw(), u32::try_from(CAPACITY)?)?,
    };
    let mut channel = Channel::setup(ports.bus, ports.memory, rings).await?;
    channel
        .submit(Command {
            id: 7,
            source: source.endpoint,
            destination: destination.endpoint,
        })
        .await?;

    let (completion, received) = oneshot::channel();
    ports.interrupts.register(
        (),
        CompletionHandler {
            channel,
            completion: Some(completion),
        },
    )?;
    let (response, fault) = received.await?;
    Ok(Outcome {
        response,
        fault,
        destination: destination.segments,
    })
}
