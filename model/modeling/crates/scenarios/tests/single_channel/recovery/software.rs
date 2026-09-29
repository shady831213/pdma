//! Complete CPU-local programs: allocation, ISR registration, draining and reset.

use super::Scenario;
use crate::harness::TestResult;
use hardware_interface_runtime::{
    InterruptHandler, InterruptRegistration, MemoryAccess, MemoryAddress, MemoryAlignment,
    RegisterAccess, SharedMemoryAccess, SharedMemoryAllocator,
};
use pdma_model_contract::dma::{
    ChannelFatalCode, ChannelId, ChannelIrqCauseValues, ChannelIrqMaskValues, Command,
    CompletionFlagsValues, Endpoint, Response,
};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Irq};
use std::{convert::Infallible, error::Error};
use tokio::sync::mpsc;

pub(super) struct Ports<'a, B> {
    pub(super) bus: B,
    pub(super) memory: SharedMemoryAccess,
    pub(super) allocator: SharedMemoryAllocator,
    pub(super) interrupts: InterruptRegistration<'a, Irq, Handler>,
}

pub(super) struct Outcome {
    pub(super) responses: Vec<Response>,
    pub(super) causes: Vec<ChannelIrqCauseValues>,
    pub(super) destination: MemoryAddress,
    pub(super) fatal: ChannelFatalCode,
}

impl Outcome {
    async fn wait_and_drain<B>(
        &mut self,
        channel: &mut Channel<B, SharedMemoryAccess>,
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
        while let Some(response) = channel.response().await? {
            self.responses.push(response);
        }
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
    const CAPACITY: u32 = 4;
    let alignment = MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?;
    let command = ports.allocator.allocate(
        u64::from(CAPACITY) * u64::try_from(Command::RECORD_SIZE)?,
        alignment,
    )?;
    let response = ports.allocator.allocate(
        u64::from(CAPACITY) * u64::try_from(Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
    )?;
    let rings = Rings {
        command: RingConfig::new(command.raw(), CAPACITY)?,
        response: RingConfig::new(response.raw(), CAPACITY)?,
    };
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
        responses: Vec::new(),
        causes: Vec::new(),
        destination,
        fatal: ChannelFatalCode::None,
    };
    let request = Command {
        source: Endpoint::linear(source.raw(), 0),
        destination: Endpoint::linear(destination.raw(), 0),
        copy_length: length,
        ..Command::ZERO
    };
    match scenario {
        Scenario::Batch => {
            channel
                .set_irq_mask(ChannelIrqMaskValues::default().with_response_non_empty(true)?)
                .await?;
            for index in 0..3 {
                channel
                    .submit(Command {
                        id: 40 + index,
                        destination: Endpoint::linear(destination.raw() + index * length, 0),
                        flags: CompletionFlagsValues { ioc: index == 2 },
                        ..request
                    })
                    .await?;
            }
        }
        Scenario::ChannelReset => {
            channel
                .set_irq_mask(
                    ChannelIrqMaskValues::default()
                        .with_response_non_empty(true)?
                        .with_ioc(true)?,
                )
                .await?;
            channel
                .submit(Command {
                    id: 41,
                    destination: Endpoint::scatter_gather(0, 0),
                    ..request
                })
                .await?;
        }
    }
    outcome
        .wait_and_drain(&mut channel, &mut notifications)
        .await?;
    match scenario {
        Scenario::Batch => {}
        Scenario::ChannelReset => {
            outcome.fatal = channel.fatal().await?;
            channel.reset().await?;
            channel = Channel::setup(ports.bus, ports.memory, config).await?;
            channel.submit(Command { id: 42, ..request }).await?;
            outcome
                .wait_and_drain(&mut channel, &mut notifications)
                .await?;
        }
    }
    Ok(outcome)
}
