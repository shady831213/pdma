//! Retained device operations survive async driving cancellation.

pub mod support;

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    InterruptSource, MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::dma::{Command, CommandRegisters, Endpoint, Response, ResponseStatus};
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

#[tokio::test]
async fn accepted_copy_completes_once_when_async_driver_is_cancelled_and_resumed()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a command and an operation retained independently of its driving future.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Command::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            1,
        )?,
        response: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Response::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            1,
        )?,
    };
    let source = allocator.allocate(8, alignment)?;
    let destination = allocator.allocate(8, alignment)?;
    let memory = allocator.memory_access();
    memory.write(source, alignment, b"retained")?;
    let device = Dma::new(memory.clone(), 1)?;
    let mut stage = support::single_command(&device);
    let model = Model::new(device);
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory.clone(),
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.submit(Command {
        sg_context: 0,
        flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
        id: 11,
        source: Endpoint::linear(source.raw(), 8),
        destination: Endpoint::linear(destination.raw(), 8),
    }))
    .await?;
    let executor = AsyncExecutor::new(model.clone());
    let mut operation = Operation::new(());
    let consumer_offset = u32::try_from(
        (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::COMMAND.offset())
            + CommandRegisters::CONSUMER.offset(),
    )?;

    // When: the driver is dropped after acceptance and the same operation is resumed.
    tokio::select! {
        biased;
        observed = async {
            loop {
                let mut consumer = [0; 4];
                model.read(consumer_offset, &mut consumer).await?;
                if u32::from_le_bytes(consumer) == 1 { return Ok::<(), pdma_model_dma::Error>(()); }
                tokio::task::yield_now().await;
            }
        } => observed?,
        result = executor.run(&mut stage, &mut operation) => assert!(result.is_err(), "driver completed before cancellation"),
    }
    assert!(!operation.is_complete());
    assert!(executor.run(&mut stage, &mut operation).await.is_ok());

    // Then: the accepted descriptor yields one response and the correct copied payload.
    assert_eq!(
        cpu.run_main(channel.response()).await?,
        Some(Response {
            flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
            id: 11,
            status: ResponseStatus::Success,
            bytes_completed: 8,
        })
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"retained");
    assert!(model.irq());
    assert!(model.clone().claim(()).await?.is_some());
    assert!(model.clone().claim(()).await?.is_none());
    assert!(!model.irq());
    Ok(())
}
