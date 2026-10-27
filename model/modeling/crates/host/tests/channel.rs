//! End-to-end host submission and interrupt notification behavior.

use pdma_model_contract::dma::{
    ChannelFatalCode, Command, CommandHeader, CopyCommandDestinationValues,
    CopyCommandSourceValues, ErrorInfo, Event, EventCode, EventInfoValues, FenceCommand,
    LinearEndpoint, Opcode, TrapCommand, WordTarget, WordTargetWidth, WordWrite,
};

use hardware_interface_irq::InterruptSource;
use hardware_interface_memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory, SharedMemoryAllocator,
};
use hardware_interface_pipeline::{AsyncExecutor, HardwareDriver};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

#[tokio::test]
async fn malformed_event_is_not_consumed_before_explicit_decode_succeeds()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: hardware has published a event, then its backing bytes are corrupted.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let event_address = allocator.allocate(
        u64::try_from(Event::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
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
        event: RingConfig::new(event_address.raw(), 1)?,
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
    cpu.run_main(
        channel.submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 13,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0,
                offset: 0,
            }),
            copy_length: 1,
            sg_context: 0,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        })),
    )
    .await?;
    until_irq(model.clone(), &mut hardware).await?;
    let expected = Event {
        id: 13,
        code: EventCode::ReadError,
        info: EventInfoValues::ReadError(ErrorInfo { bytes_completed: 0 }),
    };
    let mut corrupted = expected.to_bytes()?;
    *corrupted.get_mut(8).ok_or("event status byte missing")? = 255;
    memory.write(event_address, alignment, &corrupted)?;
    // When: software attempts to consume the malformed record.
    let result = cpu.run_main(channel.event()).await;
    // Then: codec rejection leaves the entry and IRQ pending; repairing it allows consumption.
    assert!(matches!(
        result,
        Err(pdma_model_host::DmaError::Codec(
            hardware_interface_schema::SchemaCodecError::UnknownTag { observed: 255, .. }
        ))
    ));
    assert!(model.irq());
    memory.write(event_address, alignment, &expected.to_bytes()?)?;
    assert_eq!(cpu.run_main(channel.event()).await?, Some(expected));
    assert!(model.irq());
    assert!(model.clone().claim(()).await?.is_some());
    assert!(model.clone().claim(()).await?.is_none());
    assert!(!model.irq());
    Ok(())
}

#[tokio::test]
async fn copy_fence_then_trap_exposes_completion_and_payload()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: externally allocated rings and payload memory.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(4 * Command::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            4,
        )?,
        event: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(2 * Event::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            2,
        )?,
    };
    let source = allocator.allocate(8, alignment)?;
    let destination = allocator.allocate(8, alignment)?;
    let completion = allocator.allocate(8, alignment)?;
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
    cpu.run_main(
        channel.submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 7,
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
    cpu.run_main(channel.submit(Command::Fence(FenceCommand {
        header: CommandHeader {
            opcode: Opcode::Fence,
        },
        id: 8,
        word: WordWrite {
            target: WordTarget {
                address: completion.raw(),
                context: 0,
                width: WordTargetWidth::Bits64,
            },
            value: 7,
        },
    })))
    .await?;
    cpu.run_main(channel.submit(Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Trap,
        },
        id: 9,
    })))
    .await?;
    until_irq(model.clone(), &mut hardware).await?;
    model.wait_ready().await?;
    assert!(model.claim(()).await?.is_some());
    let event = cpu.run_main(channel.event()).await?;

    // Then: Fence, payload and claimed notification agree.
    assert_eq!(event, None);
    assert_eq!(
        memory.read_typed::<[u8; 8]>(completion, alignment)?,
        7_u64.to_le_bytes()
    );
    let mut copied = [0; 8];
    memory.read(destination, alignment, &mut copied)?;
    assert_eq!(&copied, b"dma-copy");
    assert!(model.claim(()).await?.is_none());
    assert!(!model.irq());
    Ok(())
}

mod channel {
    use super::*;
    mod faults;
}

async fn until_irq(
    model: Model<hardware_interface_memory::SharedMemoryAccess>,
    hardware: &mut pdma_model_dma::Hardware,
) -> Result<(), Box<dyn std::error::Error>> {
    let executor = AsyncExecutor::new(model.clone());
    let mut driver = HardwareDriver::new();
    tokio::select! {
        result = executor.drive(hardware, &mut driver) => {
            Err(format!("hardware stopped: {result:?}").into())
        }
        result = async {
            for _ in 0..512 {
                if model.irq() { return Ok(()); }
                tokio::task::yield_now().await;
            }
            Err("interrupt watchdog expired".into())
        } => result,
    }
}
