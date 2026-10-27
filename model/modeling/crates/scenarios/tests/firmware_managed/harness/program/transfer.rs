//! Software allocation, submission and interrupt-driven Fence observation.

use pdma_model_contract::dma::{
    ChannelId, Command, CommandHeader, CopyCommandDestinationValues, CopyCommandSourceValues,
    Event, LinearEndpoint, Opcode, TrapCommand,
};

use super::super::runtime::{AccessError, HostBus};
use hardware_interface_irq::InterruptHandler;
use hardware_interface_memory::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess};
use pdma_model_host::{Channel, ChannelConfig, DmaError, RingConfig, Rings, runtime::Irq};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

use super::{Ports, TestResult};

/// Persistent Host owner of error events and Fence observation.
#[derive(Debug)]
pub struct Handler {
    channel: Channel<HostBus<SharedMemoryAccess>, SharedMemoryAccess>,
    completed: Option<oneshot::Sender<Result<u64, String>>>,
    memory: SharedMemoryAccess,
    fence: pdma_model_software::dma::CompletionWord,
    deliveries: Arc<AtomicUsize>,
}

impl InterruptHandler<Irq> for Handler {
    type Error = DmaError<AccessError>;

    async fn handle(&mut self, _: &Irq) -> Result<(), Self::Error> {
        self.deliveries.fetch_add(1, Ordering::SeqCst);
        let event = self.channel.event().await?;
        let fatal = self.channel.fatal().await?;
        self.channel.acknowledge_trap().await?;
        let result = if event.is_some() || fatal != pdma_model_contract::dma::ChannelFatalCode::None
        {
            Err(format!("DMA failed: {event:?}, {fatal:?}"))
        } else if self
            .fence
            .is_complete(&self.memory)
            .map_err(DmaError::Memory)?
        {
            Ok(self.fence.0.value)
        } else {
            return Ok(());
        };
        if let Some(completed) = self.completed.take() {
            let _closed = completed.send(result);
        }
        Ok(())
    }
}

pub(super) async fn run(
    ports: Ports<'_>,
    payload: &[u8],
    id: ChannelId,
) -> TestResult<(u64, MemoryAddress, Arc<AtomicUsize>)> {
    let command = ports.allocator.allocate(
        u64::try_from(4 * Command::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
    )?;
    let response = ports.allocator.allocate(
        u64::try_from(2 * Event::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
    )?;
    let length = u64::try_from(payload.len())?;
    let source = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
    let destination = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
    ports.memory.write(source, MemoryAlignment::ONE, payload)?;
    ports
        .memory
        .write(destination, MemoryAlignment::ONE, &vec![0; payload.len()])?;
    let dma_base = ports.bus.addresses().dma;
    let mut channel = Channel::setup_at(
        ports.bus,
        ports.memory.clone(),
        ChannelConfig {
            id,
            rings: Rings {
                command: RingConfig::new(command.raw(), 4)?,
                event: RingConfig::new(response.raw(), 2)?,
            },
        },
        dma_base,
    )
    .await?;
    channel
        .submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 37,
            copy_length: length,
            sg_context: 0,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: source.raw(),
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: destination.raw(),
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }))
        .await?;
    let address = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
    ports
        .memory
        .write(address, MemoryAlignment::ONE, &0_u64.to_le_bytes())?;
    let fence = pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
        target: pdma_model_contract::dma::WordTarget {
            address: address.raw(),
            context: 0,
            width: pdma_model_contract::dma::WordTargetWidth::Bits64,
        },
        value: 37,
    });
    channel.submit(fence.command(38)).await?;
    channel
        .submit(Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 39,
        }))
        .await?;
    let (completed, received) = oneshot::channel();
    let deliveries = Arc::new(AtomicUsize::new(0));
    ports.interrupts.register(
        super::HostVector::Dma,
        super::Handler::Dma(Box::new(Handler {
            channel,
            memory: ports.memory,
            fence,
            completed: Some(completed),
            deliveries: Arc::clone(&deliveries),
        })),
    )?;
    Ok((received.await??, destination, deliveries))
}
