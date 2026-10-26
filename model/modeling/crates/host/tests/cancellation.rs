//! Retained device operations survive async driving cancellation.

use pdma_model_contract::dma::{
    Command, CommandRegisters, CopyCommandDestinationValues, CopyCommandSourceValues, Event,
    LinearEndpoint,
};

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
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
        event: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Event::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
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
    let mut stage = device.hardware();
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
    cpu.run_main(
        channel.submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 11,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: source.raw(),
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: destination.raw(),
                offset: 0,
            }),
            copy_length: 8,
            sg_context: 0,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        })),
    )
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
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    assert_eq!(
        memory.read_typed::<[u8; 8]>(destination, alignment)?,
        [0; 8]
    );
    assert!(executor.run(&mut stage, &mut operation).await.is_ok());

    // Then: the accepted descriptor completes silently with the correct copied payload.
    assert!(operation.is_complete());
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"retained");
    assert!(!model.irq());
    assert!(!model.irq());
    Ok(())
}
