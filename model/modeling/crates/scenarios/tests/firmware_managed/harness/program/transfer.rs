//! Software allocation, submission and interrupt-driven response consumption.

use super::super::runtime::{AccessError, HostBus};
use hardware_interface_runtime::{
    InterruptHandler, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_contract::{ChannelId, Command, CompletionFlagsValues, Endpoint, Response};
use pdma_model_host::{Channel, ChannelConfig, Error, RingConfig, Rings, runtime::Irq};
use tokio::sync::oneshot;

use super::{Ports, TestResult};

/// Persistent Host owner of response consumption.
#[derive(Debug)]
pub struct Handler {
    channel: Channel<HostBus<SharedMemoryAccess>, SharedMemoryAccess>,
    completed: Option<oneshot::Sender<Response>>,
}

impl InterruptHandler<Irq> for Handler {
    type Error = Error<AccessError>;

    async fn handle(&mut self, _: &Irq) -> Result<(), Self::Error> {
        if let Some(response) = self.channel.response().await? {
            if let Some(completed) = self.completed.take() {
                let _closed = completed.send(response);
            }
        }
        Ok(())
    }
}

pub(super) async fn run(
    ports: Ports<'_>,
    payload: &[u8],
    id: ChannelId,
) -> TestResult<(Response, MemoryAddress)> {
    let command = ports.allocator.allocate(
        u64::try_from(2 * Command::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
    )?;
    let response = ports.allocator.allocate(
        u64::try_from(2 * Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
    )?;
    let length = u64::try_from(payload.len())?;
    let source = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
    let destination = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
    ports.memory.write(source, MemoryAlignment::ONE, payload)?;
    ports
        .memory
        .write(destination, MemoryAlignment::ONE, &vec![0; payload.len()])?;
    let mut channel = Channel::setup(
        ports.bus,
        ports.memory,
        ChannelConfig {
            id,
            rings: Rings {
                command: RingConfig::new(command.raw(), 2)?,
                response: RingConfig::new(response.raw(), 2)?,
            },
        },
    )
    .await?;
    channel
        .submit(Command {
            id: 37,
            source: Endpoint::linear(source.raw(), length),
            destination: Endpoint::linear(destination.raw(), length),
            flags: CompletionFlagsValues::default(),
        })
        .await?;
    let (completed, received) = oneshot::channel();
    ports.interrupts.register(
        (),
        Handler {
            channel,
            completed: Some(completed),
        },
    )?;
    Ok((received.await?, destination))
}
