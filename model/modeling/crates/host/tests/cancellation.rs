//! Retained device operations survive async driving cancellation.

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{Command, CommandRegisters, DeviceRegisters, Response, ResponseStatus};
use pdma_model_device::{Device, pipeline};
use pdma_model_host::{Channel, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

#[tokio::test]
async fn accepted_copy_completes_once_when_async_driver_is_cancelled_and_resumed()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a command and an operation retained independently of its driving future.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(allocator.allocate(32, alignment)?.raw(), 1)?,
        response: RingConfig::new(allocator.allocate(16, alignment)?.raw(), 1)?,
    };
    let source = allocator.allocate(8, alignment)?;
    let destination = allocator.allocate(8, alignment)?;
    let memory = allocator.memory_access();
    memory.write(source, alignment, b"retained")?;
    let model = Model::new(Device::new(memory.clone()));
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
        .await?;
    cpu.run_main(channel.submit(Command {
        id: 11,
        source: source.raw(),
        destination: destination.raw(),
        length: 8,
    }))
    .await?;
    let executor = AsyncExecutor::new(model.clone());
    let mut stage = pipeline();
    let mut operation = Operation::new(());
    let consumer_offset =
        u32::try_from(DeviceRegisters::COMMAND.offset() + CommandRegisters::CONSUMER.offset())?;

    // When: the driver is dropped after acceptance and the same operation is resumed.
    tokio::select! {
        biased;
        observed = async {
            loop {
                let mut consumer = [0; 4];
                model.read(consumer_offset, &mut consumer).await?;
                if u32::from_le_bytes(consumer) == 1 { return Ok::<(), pdma_model_device::Error>(()); }
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
            id: 11,
            status: ResponseStatus::Success
        })
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"retained");
    assert!(!model.irq());
    Ok(())
}
