//! Complete CPU-local programs: allocation, ISR registration, draining and reset.

use hw_model_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    ChannelFatalCode, ChannelId, ChannelIrqCauseValues, ChannelIrqMaskValues, Command,
    CommandHeader, CopyCommand, CopyCommandDestinationValues, CopyCommandSourceValues, Event,
    LinearEndpoint, Opcode, ScatterGatherEndpoint, TrapCommand,
};

mod submission;

use super::Scenario;
use crate::harness::TestResult;
use hw_model_io::memory::SharedMemoryAllocator;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess};
use hw_model_io::register::RegisterAccess;
use hw_model_irq::InterruptHandler;
use hw_model_swenv::InterruptRegistration;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Irq};
use std::{convert::Infallible, error::Error};
use tokio::sync::mpsc;

pub(super) struct Ports<'a, B> {
    pub(super) bus: B,
    pub(super) memory: SharedMemoryAccess<(), MemoryAccessError>,
    pub(super) allocator: SharedMemoryAllocator,
    pub(super) interrupts: InterruptRegistration<'a, Irq, Handler>,
}

pub(super) struct Outcome {
    pub(super) events: Vec<Event>,
    pub(super) completed: u64,
    pub(super) causes: Vec<ChannelIrqCauseValues>,
    pub(super) destination: MemoryAddress,
    pub(super) fatal: ChannelFatalCode,
}

impl Outcome {
    async fn wait_and_drain<B>(
        &mut self,
        channel: &mut Channel<B, SharedMemoryAccess<(), MemoryAccessError>>,
        notifications: &mut mpsc::Receiver<()>,
    ) -> TestResult
    where
        B: RegisterAccess + Send + Sync,
        B::Error: Error + Send + Sync + 'static,
    {
        notifications
            .recv()
            .await
            .ok_or("interrupt channel closed")?;
        self.causes.push(channel.irq_cause().await?);
        while let Some(response) = channel.event().await? {
            self.events.push(response);
        }
        channel.acknowledge_trap().await?;
        Ok(())
    }
}

pub(super) struct Handler(mpsc::Sender<()>);

impl InterruptHandler<Irq> for Handler {
    type Error = Infallible;
    async fn handle(&mut self, _: &Irq) -> Result<(), Infallible> {
        let _receiver_closed = self.0.send(()).await;
        Ok(())
    }
}

pub(super) async fn main<B>(
    ports: Ports<'_, B>,
    scenario: Scenario,
    payload: &[u8],
) -> TestResult<Outcome>
where
    B: RegisterAccess + Clone + Send + Sync,
    B::Error: Error + Send + Sync + 'static,
{
    let rings = allocate_rings(&ports.allocator)?;
    let length = u64::try_from(payload.len())?;
    let source = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
    let copies = match scenario {
        Scenario::Batch => 3,
        Scenario::ChannelReset => 1,
    };
    let destination = ports
        .allocator
        .allocate(length * copies, MemoryAlignment::ONE)?;
    ports.memory.write(source, MemoryAlignment::ONE, payload)?;
    let (notice, mut notifications) = mpsc::channel(4);
    ports.interrupts.register((), Handler(notice))?;
    let config = ChannelConfig {
        id: ChannelId::new(0),
        rings,
    };
    let mut channel = Channel::setup(ports.bus.clone(), ports.memory.clone(), config).await?;
    let mut outcome = Outcome {
        events: Vec::new(),
        completed: 0,
        causes: Vec::new(),
        destination,
        fatal: ChannelFatalCode::None,
    };
    let fence = completion(&ports.allocator)?;
    let request = pdma_model_contract::dma::CopyCommand {
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: source.raw(),
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: destination.raw(),
            offset: 0,
        }),
        copy_length: length,
        sg_context: 0,
        ..pdma_model_contract::dma::CopyCommand::ZERO
    };
    submission::initial(&mut channel, scenario, (request, fence)).await?;
    outcome
        .wait_and_drain(&mut channel, &mut notifications)
        .await?;
    match scenario {
        Scenario::Batch => {}
        Scenario::ChannelReset => {
            outcome.fatal = channel.fatal().await?;
            channel.reset().await?;
            channel = Channel::setup(ports.bus, ports.memory.clone(), config).await?;
            channel
                .submit(Command::Copy(CopyCommand { id: 42, ..request }))
                .await?;
            channel.submit(fence.command(43)).await?;
            channel
                .submit(Command::Trap(TrapCommand {
                    header: CommandHeader {
                        opcode: Opcode::Trap,
                    },
                    id: 44,
                }))
                .await?;
            outcome
                .wait_and_drain(&mut channel, &mut notifications)
                .await?;
        }
    }
    if !fence.is_complete(&ports.memory)? {
        return Err("Trap without qualified Fence".into());
    }
    outcome.completed = fence.0.value;
    Ok(outcome)
}

fn allocate_rings(allocator: &SharedMemoryAllocator) -> TestResult<Rings> {
    const CAPACITY: u32 = 8;
    let alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let command = allocator.allocate(
        u64::from(CAPACITY) * u64::try_from(Command::RECORD_SIZE)?,
        alignment,
    )?;
    let response = allocator.allocate(
        u64::from(CAPACITY) * u64::try_from(Event::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
    )?;
    Ok(Rings {
        command: RingConfig::new(command.raw(), CAPACITY)?,
        event: RingConfig::new(response.raw(), CAPACITY)?,
    })
}

fn completion(
    allocator: &SharedMemoryAllocator,
) -> TestResult<pdma_model_software::dma::CompletionWord> {
    let fence_address = allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
    allocator
        .memory_access()
        .write(fence_address, MemoryAlignment::ONE, &0_u64.to_le_bytes())?;
    let fence = pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
        target: pdma_model_contract::dma::WordTarget {
            address: fence_address.raw(),
            context: 0,
            width: pdma_model_contract::dma::WordTargetWidth::Bits64,
        },
        value: 42,
    });
    Ok(fence)
}
