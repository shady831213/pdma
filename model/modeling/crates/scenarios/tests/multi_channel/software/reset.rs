use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::dma::{ChannelBlock, CommandRegisters, ResponseRegisters};
use std::error::Error;

use super::{Ports, buffers::Buffers, read_word};
use crate::{Scenario, TestResult};

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
        let snapshot = first.snapshot(&ports.memory)?;
        let ready = match scenario {
            Scenario::CopyInFlight | Scenario::QueuedChannel | Scenario::WholeDevice => {
                snapshot.first() == Some(&first.value)
            }
            Scenario::ResponseBackpressure => {
                buffers
                    .iter()
                    .skip(1)
                    .try_fold(true, |ready, buffer| -> TestResult<bool> {
                        Ok(ready
                            && buffer.snapshot(&ports.memory)?.get(buffer.length - 1)
                                == Some(&buffer.value))
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
        Scenario::ResponseBackpressure => {
            let base = first
                .config
                .id
                .register_offset(3)
                .ok_or("missing channel registers")?;
            let producer = read_word(
                &ports.bus,
                base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset(),
            )
            .await?;
            let consumer = command_consumer(ports, first).await?;
            Ok(Some((producer, consumer)))
        }
    }
}
