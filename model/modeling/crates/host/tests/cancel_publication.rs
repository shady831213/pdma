//! Cancellation after a committed DMA write preserves publication state.

pub mod support;

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    InterruptSource, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{Command, Endpoint, Response, ResponseStatus};
use pdma_model_dma::Device;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

#[tokio::test]
async fn committed_copy_is_not_replayed_when_cancelled_before_response_publication()
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
    memory.write(source, alignment, b"original")?;
    let device = Device::new(memory.clone(), 1)?;
    let mut stage = support::single_command(&device);
    let model = Model::new(device);
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory.clone(),
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.submit(Command {
        flags: pdma_model_contract::CompletionFlagsValues::default(),
        id: 23,
        source: Endpoint::linear(source.raw(), 8),
        destination: Endpoint::linear(destination.raw(), 8),
    }))
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
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    memory.write(source, alignment, b"new-data")?;
    memory.write(destination, alignment, b"sentinel")?;
    assert!(executor.run(&mut stage, &mut operation).await.is_ok());

    // Then: publication completes exactly once without replaying either DMA step.
    assert!(model.irq());
    assert_eq!(
        cpu.run_main(channel.response()).await?,
        Some(Response {
            flags: pdma_model_contract::CompletionFlagsValues::default(),
            id: 23,
            status: ResponseStatus::Success,
            bytes_completed: 8,
        })
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"sentinel");
    assert!(model.irq());
    assert!(model.clone().claim(()).await?.is_some());
    assert!(model.clone().claim(()).await?.is_none());
    assert!(!model.irq());
    Ok(())
}
