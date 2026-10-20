//! End-to-end host submission and interrupt notification behavior.

use hardware_interface_pipeline::{AsyncExecutor, HardwareDriver};
use hardware_interface_runtime::{
    InterruptSource, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::dma::{
    ChannelFatalCode, Command, CopyCommand, Endpoint, Response, ResponseStatus,
};
use pdma_model_contract::dma::{CompletionInfo, ResponseInfoValues};
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
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
    let device = Dma::new(memory.clone(), 1)?;
    let mut hardware = device.hardware();
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
    cpu.run_main(channel.submit(Command::copy(
        13,
        CopyCommand {
            copy_length: 0,
            sg_context: 0,
            source: Endpoint::linear(0, 0),
            destination: Endpoint::linear(0, 0),
        },
    )))
    .await?;
    until_irq(model.clone(), &mut hardware).await?;
    assert!(model.irq(), "response did not arrive before watchdog");
    let expected = Response {
        flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
        id: 13,
        status: ResponseStatus::Success,
        info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 0 }),
    };
    let mut corrupted = expected.to_bytes()?;
    *corrupted.get_mut(8).ok_or("response status byte missing")? = 255;
    memory.write(response_address, alignment, &corrupted)?;
    // When: software attempts to consume the malformed record.
    let result = cpu.run_main(channel.response()).await;
    // Then: codec rejection leaves the entry and IRQ pending; repairing it allows consumption.
    assert!(matches!(
        result,
        Err(pdma_model_host::DmaError::Codec(
            hardware_interface_schema::SchemaCodecError::UnknownTag { observed: 255, .. }
        ))
    ));
    assert!(model.irq());
    memory.write(response_address, alignment, &expected.to_bytes()?)?;
    assert_eq!(cpu.run_main(channel.response()).await?, Some(expected));
    assert!(model.irq());
    assert!(model.clone().claim(()).await?.is_some());
    assert!(model.clone().claim(()).await?.is_none());
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
    let device = Dma::new(memory.clone(), 1)?;
    let mut hardware = device.hardware();
    let mut model = Model::new(device);
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

    // When: a real async worker executes the submitted command.
    cpu.run_main(channel.submit(Command::copy(
        7,
        CopyCommand {
            copy_length: 8,
            sg_context: 0,
            source: Endpoint::linear(source.raw(), 0),
            destination: Endpoint::linear(destination.raw(), 0),
        },
    )))
    .await?;
    until_irq(model.clone(), &mut hardware).await?;
    assert!(model.irq(), "response did not arrive before watchdog");
    model.wait_ready().await?;
    assert!(model.claim(()).await?.is_some());
    let response = cpu.run_main(channel.response()).await?;

    // Then: response, payload and claimed notification agree.
    assert_eq!(
        response,
        Some(Response {
            flags: pdma_model_contract::dma::CompletionFlagsValues::default(),
            id: 7,
            status: ResponseStatus::Success,
            info: ResponseInfoValues::Success(CompletionInfo { bytes_completed: 8 }),
        })
    );
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"dma-copy");
    assert!(model.claim(()).await?.is_none());
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
    let device = Dma::new(memory.clone(), 1)?;
    let mut hardware = device.hardware();
    let mut model = Model::new(device);
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.submit(Command::copy(
        9,
        CopyCommand {
            copy_length: 8,
            sg_context: 0,
            source: Endpoint::linear(source.raw(), 0),
            destination: Endpoint::linear(destination.raw(), 0),
        },
    )))
    .await?;

    // When: response publication fails at the real memory boundary.
    until_irq(model.clone(), &mut hardware).await?;
    assert!(
        model.irq(),
        "fatal notification did not arrive before watchdog"
    );

    // Then: the notification is claimed once while the fatal remains visible through masking.
    model.wait_ready().await?;
    assert!(model.claim(()).await?.is_some());
    assert_eq!(
        cpu.run_main(channel.fatal()).await?,
        ChannelFatalCode::ResponseWrite
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    cpu.run_main(channel.set_irq_enabled(false)).await?;
    assert!(model.claim(()).await?.is_none());
    assert!(!model.irq());
    assert_eq!(
        cpu.run_main(channel.fatal()).await?,
        ChannelFatalCode::ResponseWrite
    );
    Ok(())
}

async fn until_irq(
    model: Model<hardware_interface_runtime::SharedMemoryAccess>,
    hardware: &mut pdma_model_dma::Hardware,
) -> Result<(), Box<dyn std::error::Error>> {
    let executor = AsyncExecutor::new(model.clone());
    let mut driver = HardwareDriver::new();
    tokio::select! {
        result = executor.drive(hardware, &mut driver) => {
            Err(format!("hardware stopped: {result:?}").into())
        }
        () = async {
            for _ in 0..512 {
                if model.irq() { break; }
                tokio::task::yield_now().await;
            }
        } => Ok(()),
    }
}
