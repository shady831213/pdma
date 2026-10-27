//! Cancellation after a committed DMA write preserves retirement state.

use pdma_model_contract::dma::{
    Command, CopyCommandDestinationValues, CopyCommandSourceValues, Event, LinearEndpoint,
};

use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{MemoryAccessError, SharedMemory, SharedMemoryAllocator};
use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_software::SoftwareEnv;
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

#[tokio::test]
async fn committed_copy_is_not_replayed_when_cancelled_before_retirement()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: an operation with externally observable source and destination storage.
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
    memory.write(source, alignment, b"original")?;
    let device = Dma::new(memory.clone(), 1)?;
    let mut stage = device.hardware();
    let model = Model::new(device);
    let cpu = SoftwareEnv::new();
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
            id: 23,
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

    // When: cancellation follows the DMA write, then changed buffers precede resumption.
    let copied_before_completion = tokio::select! {
        biased;
        observed = async {
            for _ in 0..64 {
                let mut copied = [0; 8];
                memory.read(destination, alignment, &mut copied)?;
                if &copied == b"original" {
                    return Ok::<bool, MemoryAccessError>(true);
                }
                tokio::task::yield_now().await;
            }
            Ok(false)
        } => observed?,
        result = executor.run(&mut stage, &mut operation) => {
            assert!(result.is_ok());
            false
        },
    };
    assert!(copied_before_completion);
    assert!(!operation.is_complete());
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    memory.write(source, alignment, b"new-data")?;
    memory.write(destination, alignment, b"sentinel")?;
    assert!(executor.run(&mut stage, &mut operation).await.is_ok());

    // Then: retirement completes exactly once without replaying either DMA step.
    assert!(!model.irq());
    assert!(operation.is_complete());
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"sentinel");
    assert!(!model.irq());
    assert!(!model.irq());
    Ok(())
}
