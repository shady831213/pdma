//! End-to-end host submission and level interrupt behavior.

use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::{
    InterruptSource, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{ChannelFaultCode, Command, Endpoint, Response, ResponseStatus};
use pdma_model_device::{Device, pipeline};
use pdma_model_host::{Channel, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

#[tokio::test]
async fn malformed_response_is_not_consumed_before_explicit_decode_succeeds()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: hardware has published a response, then its backing bytes are corrupted.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let response_address = allocator.allocate(
        u64::try_from(Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
    )?;
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
        response: RingConfig::new(response_address.raw(), 1)?,
    };
    let memory = allocator.memory_access();
    let model = Model::new(Device::new(memory.clone()));
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
        .await?;
    cpu.run_main(channel.submit(Command {
        id: 13,
        source: Endpoint::linear(0, 0),
        destination: Endpoint::linear(0, 0),
    }))
    .await?;
    let mut stage = pipeline();
    let mut operation = Operation::new(());
    assert!(
        AsyncExecutor::new(model.clone())
            .run(&mut stage, &mut operation)
            .await
            .is_ok()
    );
    let expected = Response {
        id: 13,
        status: ResponseStatus::Success,
        bytes_completed: 0,
    };
    let mut corrupted = expected.to_bytes()?;
    *corrupted.get_mut(8).ok_or("response status byte missing")? = 255;
    memory.write(response_address, alignment, &corrupted)?;
    // When: software attempts to consume the malformed record.
    let result = cpu.run_main(channel.response()).await;
    // Then: codec rejection leaves the entry and IRQ pending; repairing it allows consumption.
    assert!(matches!(
        result,
        Err(pdma_model_host::Error::Codec(
            hardware_interface_schema::SchemaCodecError::UnknownTag { observed: 255, .. }
        ))
    ));
    assert!(model.irq());
    memory.write(response_address, alignment, &expected.to_bytes()?)?;
    assert_eq!(cpu.run_main(channel.response()).await?, Some(expected));
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn copy_completes_when_host_submits_and_consumes_response()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: externally allocated rings and payload memory.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(2 * Command::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            2,
        )?,
        response: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(2 * Response::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            2,
        )?,
    };
    let source = allocator.allocate(8, alignment)?;
    let destination = allocator.allocate(8, alignment)?;
    let memory = allocator.memory_access();
    memory.write(source, alignment, b"dma-copy")?;
    let mut model = Model::new(Device::new(memory.clone()));
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
        .await?;
    let mut stage = pipeline();
    let mut operation = Operation::new(());
    let executor = AsyncExecutor::new(model.clone());

    // When: a real async worker executes the submitted command.
    cpu.run_main(channel.submit(Command {
        id: 7,
        source: Endpoint::linear(source.raw(), 8),
        destination: Endpoint::linear(destination.raw(), 8),
    }))
    .await?;
    assert!(executor.run(&mut stage, &mut operation).await.is_ok());
    model.wait_ready().await?;
    assert!(model.claim(()).await?.is_some());
    let response = cpu.run_main(channel.response()).await?;

    // Then: response, payload and interrupt deassertion agree.
    assert_eq!(
        response,
        Some(Response {
            id: 7,
            status: ResponseStatus::Success,
            bytes_completed: 8,
        })
    );
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"dma-copy");
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn fault_interrupt_arrives_when_response_write_is_unmapped()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the response ring points outside the injected memory backend.
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
        response: RingConfig::new(0x8000, 1)?,
    };
    let source = allocator.allocate(8, alignment)?;
    let destination = allocator.allocate(8, alignment)?;
    let memory = allocator.memory_access();
    let mut model = Model::new(Device::new(memory.clone()));
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory, rings))
        .await?;
    cpu.run_main(channel.submit(Command {
        id: 9,
        source: Endpoint::linear(source.raw(), 8),
        destination: Endpoint::linear(destination.raw(), 8),
    }))
    .await?;
    let mut stage = pipeline();
    let mut operation = Operation::new(());
    let executor = AsyncExecutor::new(model.clone());

    // When: response publication fails at the real memory boundary.
    assert!(executor.run(&mut stage, &mut operation).await.is_err());

    // Then: sticky fault remains visible and software can mask its level IRQ.
    model.wait_ready().await?;
    assert!(model.claim(()).await?.is_some());
    assert_eq!(
        cpu.run_main(channel.fault()).await?,
        ChannelFaultCode::ResponseWrite
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    cpu.run_main(channel.set_irq_enabled(false)).await?;
    assert!(!model.irq());
    assert_eq!(
        cpu.run_main(channel.fault()).await?,
        ChannelFaultCode::ResponseWrite
    );
    Ok(())
}
