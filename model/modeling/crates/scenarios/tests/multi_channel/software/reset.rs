use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::dma::{ChannelBlock, CommandRegisters, EventRegisters};
use std::error::Error;

use super::{Ports, buffers::Buffers, read_word};
use crate::{Scenario, TestResult};

#[cfg(test)]
mod tests;

pub(super) async fn command_consumer<B>(ports: &Ports<'_, B>, buffer: &Buffers) -> TestResult<u32>
where
    B: RegisterAccess + Send + Sync,
    B::Error: Error + Send + Sync + 'static,
{
    let base = buffer
        .config
        .id
        .register_offset(3)
        .ok_or("missing channel registers")?;
    read_word(
        &ports.bus,
        base + ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset(),
    )
    .await
}

pub(super) async fn wait_boundary<B>(
    ports: &Ports<'_, B>,
    buffers: &[Buffers],
    scenario: Scenario,
) -> TestResult<Option<(u32, u32)>>
where
    B: RegisterAccess + Send + Sync,
    B::Error: Error + Send + Sync + 'static,
{
    let first = buffers.first().ok_or("missing channel A")?;
    loop {
        let ready = match scenario {
            Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::WholeDevice => {
                first.byte_matches(&ports.memory, 0)?
            }
            Scenario::SuccessWithoutEvents => {
                buffers
                    .iter()
                    .try_fold(true, |ready, buffer| -> TestResult<bool> {
                        Ok(ready && buffer.fence.is_complete(&ports.memory)?)
                    })?
            }
        };
        if ready {
            break;
        }
        tokio::task::yield_now().await;
    }
    match scenario {
        Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::WholeDevice => Ok(None),
        Scenario::SuccessWithoutEvents => {
            let base = first
                .config
                .id
                .register_offset(3)
                .ok_or("missing channel registers")?;
            let producer = read_word(
                &ports.bus,
                base + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset(),
            )
            .await?;
            let consumer = command_consumer(ports, first).await?;
            Ok(Some((producer, consumer)))
        }
    }
}
