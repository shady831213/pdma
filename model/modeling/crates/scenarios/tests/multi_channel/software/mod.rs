//! Software owns allocation, shared IRQ draining and channel reset through the bus.

mod buffers;
mod reset;

use hardware_interface_runtime::{
    InterruptHandler, InterruptRegistration, MemoryAddress, RegisterAccess, SharedMemoryAccess,
    SharedMemoryAllocator,
};
use pdma_model_contract::{ChannelId, DeviceControl, DeviceRegisters, Response};
use pdma_model_host::{Channel, runtime::Irq};
use std::{convert::Infallible, error::Error};
use tokio::sync::mpsc;

use crate::{Scenario, TestResult};
use buffers::Buffers;

pub(super) struct Ports<'a, B> {
    pub(super) bus: B,
    pub(super) memory: SharedMemoryAccess,
    pub(super) allocator: SharedMemoryAllocator,
    pub(super) interrupts: InterruptRegistration<'a, Irq, Handler>,
}

pub(super) struct CopyResult {
    pub(super) address: MemoryAddress,
    pub(super) length: usize,
    pub(super) value: u8,
    pub(super) before_reset: Vec<u8>,
    pub(super) command_consumer: u32,
}

pub(super) struct Outcome {
    pub(super) responses: Vec<(ChannelId, Response)>,
    pub(super) original: Vec<CopyResult>,
    pub(super) fresh: MemoryAddress,
    pub(super) channel_counts: [u32; 2],
    pub(super) discovery_write_rejected: bool,
    pub(super) blocked_cursors: Option<(u32, u32)>,
}

pub(super) struct Handler(mpsc::Sender<()>);

impl InterruptHandler<Irq> for Handler {
    type Error = Infallible;

    async fn handle(&mut self, _: &Irq) -> Result<(), Infallible> {
        let _receiver_closed = self.0.send(()).await;
        Ok(())
    }
}

async fn read_word<B: RegisterAccess + Sync>(bus: &B, offset: usize) -> TestResult<u32>
where
    B::Error: Error + Send + Sync + 'static,
{
    let mut bytes = [0; 4];
    bus.read(u32::try_from(offset)?, &mut bytes).await?;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) async fn main<B>(ports: Ports<'_, B>, scenario: Scenario) -> TestResult<Outcome>
where
    B: RegisterAccess + Clone + Send + Sync,
    B::Error: Error + Send + Sync + 'static,
{
    let count_offset = DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset();
    let initial_count = read_word(&ports.bus, count_offset).await?;
    let discovery_write_rejected = ports
        .bus
        .write(u32::try_from(count_offset)?, &1_u32.to_le_bytes())
        .await
        .is_err();
    let (notice, mut notifications) = mpsc::channel(16);
    ports.interrupts.register((), Handler(notice))?;
    let mut buffers = Vec::new();
    let mut channels = Vec::new();
    for index in 0..usize::try_from(initial_count)? {
        let allocation = Buffers::allocate(&ports.allocator, index)?;
        let mut channel =
            Channel::setup(ports.bus.clone(), ports.memory.clone(), allocation.config).await?;
        channel
            .submit(allocation.command(10 + u64::try_from(index)?, false)?)
            .await?;
        match scenario {
            Scenario::ResponseBackpressure => {
                if index == 0 {
                    channel.submit(allocation.command(19, true)?).await?;
                }
            }
            Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::WholeDevice => {}
        }
        buffers.push(allocation);
        channels.push(channel);
    }
    let blocked_cursors = reset::wait_boundary(&ports, &buffers, scenario).await?;
    let mut original = Vec::new();
    for buffer in &buffers {
        original.push(CopyResult {
            address: buffer.destination,
            length: buffer.length,
            value: buffer.value,
            before_reset: buffer.snapshot(&ports.memory)?,
            command_consumer: reset::command_consumer(&ports, buffer).await?,
        });
    }
    let reset_index = match scenario {
        Scenario::QueuedChannel => 1,
        Scenario::CopyInFlight | Scenario::ResponseBackpressure | Scenario::WholeDevice => 0,
    };
    let selected = channels.remove(reset_index);
    match scenario {
        Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::ResponseBackpressure => {
            selected.reset().await?;
        }
        Scenario::WholeDevice => {
            selected.reset_device().await?;
            channels.clear();
        }
    }
    let final_count = read_word(&ports.bus, count_offset).await?;
    let fresh = Buffers::allocate(&ports.allocator, reset_index)?;
    let mut replacement =
        Channel::setup(ports.bus.clone(), ports.memory.clone(), fresh.config).await?;
    replacement.submit(fresh.command(20, false)?).await?;
    channels.insert(reset_index, replacement);
    let expected = match scenario {
        Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::ResponseBackpressure => 3,
        Scenario::WholeDevice => 1,
    };
    let mut responses = Vec::new();
    while responses.len() < expected {
        notifications
            .recv()
            .await
            .ok_or("interrupt channel closed")?;
        for (index, channel) in channels.iter_mut().enumerate() {
            let id = ChannelId::new(index);
            while let Some(response) = channel.response().await? {
                responses.push((id, response));
            }
        }
    }
    Ok(Outcome {
        responses,
        original,
        fresh: fresh.destination,
        channel_counts: [initial_count, final_count],
        discovery_write_rejected,
        blocked_cursors,
    })
}
