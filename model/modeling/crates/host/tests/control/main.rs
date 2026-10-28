//! Host interrupt controls and explicit reset recovery.

use hw_model_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    ChannelInterrupts, ChannelIrqMaskValues, ChannelRegisters, Command, CommandHeader, DmaControl,
    DmaFatalCode, Event, Opcode, TrapCommand,
};

use hw_model_io::memory::{MemoryAddress, MemoryAlignment, SharedMemoryAccess};
use hw_model_io::memory::{SharedMemory, SharedMemoryAllocator};
use hw_model_io::register::RegisterAccess;
use hw_model_pipeline::{AsyncExecutor, AsyncInteractionBackend, HardwareDriver, StepOutcome};
use hw_model_swenv::SoftwareEnv;
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

mod capabilities;
mod reset;

type TestMemory = SharedMemoryAccess<(), MemoryAccessError>;

fn fixture() -> Result<(Dma<TestMemory>, TestMemory, Rings), Box<dyn std::error::Error>> {
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )?;
    let memory = allocator.memory_access();
    let device = Dma::new(memory.clone(), 1)?;
    let rings = Rings {
        command: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Command::RECORD_SIZE)?,
                    MemoryAlignment::try_new(8)?,
                )?
                .raw(),
            1,
        )?,
        event: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Event::RECORD_SIZE)?,
                    MemoryAlignment::try_new(8)?,
                )?
                .raw(),
            1,
        )?,
    };
    Ok((device, memory, rings))
}

#[tokio::test]
async fn setup_preserves_the_device_interrupt_mask() -> Result<(), Box<dyn std::error::Error>> {
    // Given: device interrupts are masked before any channel is configured.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let cpu = SoftwareEnv::new();
    // When: software configures a channel.
    let _channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    // Then: channel setup preserves the device-wide gate.
    let mut bytes = [0; 4];
    model
        .read(
            u32::try_from(
                (pdma_model_contract::dma::DmaDiscovery::EXTENT
                    + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
                    + DmaControl::IRQ_ENABLE.offset(),
            )?,
            &mut bytes,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    Ok(())
}

#[tokio::test]
async fn software_explicitly_enables_device_and_channel_interrupts()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: disabled hardware and a software CPU.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let cpu = SoftwareEnv::new();
    // When: software programs the channel.
    let channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.set_dma_irq_enabled(true)).await?;
    // Then: both independent interrupt gates are enabled.
    for offset in [
        (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::IRQ.offset()
            + ChannelInterrupts::ENABLE.offset(),
        (pdma_model_contract::dma::DmaDiscovery::EXTENT
            + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
            + DmaControl::IRQ_ENABLE.offset(),
    ] {
        let mut bytes = [0; 4];
        model.read(u32::try_from(offset)?, &mut bytes).await?;
        assert_eq!(u32::from_le_bytes(bytes), 1);
    }
    Ok(())
}

#[tokio::test]
async fn masked_trap_cause_remains_visible_without_unmask_catch_up()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both routine notification sources are masked.
    let (device, memory, rings) = fixture()?;
    let mut hardware = device.hardware();
    let model = Model::new(device);
    let cpu = SoftwareEnv::new();
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
    cpu.run_main(channel.set_irq_mask(ChannelIrqMaskValues::new(3)))
        .await?;
    cpu.run_main(channel.submit(Command::Trap(TrapCommand {
        header: CommandHeader {
            opcode: Opcode::Trap,
        },
        id: 7,
    })))
    .await?;
    // When: hardware publishes a masked Trap and software unmasks.
    let executor = AsyncExecutor::new(model.clone());
    let mut driver = HardwareDriver::new();
    tokio::select! {
        result = executor.drive(&mut hardware, &mut driver) => {
            return Err(format!("hardware stopped: {result:?}").into());
        }
        result = async {
            for _ in 0..512 {
                if cpu.run_main(channel.irq_cause()).await?.trap() {
                    return Ok::<(), Box<dyn std::error::Error>>(());
                }
                tokio::task::yield_now().await;
            }
            Err("completion cause watchdog".into())
        } => result?,
    }
    cpu.run_main(channel.set_irq_mask(ChannelIrqMaskValues::default()))
        .await?;
    // Then: software sees and acknowledges Trap without a catch-up IRQ.
    assert!(!model.irq());
    let cause = cpu.run_main(channel.irq_cause()).await?;
    assert!(!cause.event_non_empty());
    assert!(cause.trap());
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    cpu.run_main(channel.acknowledge_trap()).await?;
    assert!(!cpu.run_main(channel.irq_cause()).await?.trap());
    Ok(())
}

#[tokio::test]
async fn dma_fatal_is_visible_after_unmask_without_replaying_notification()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: software has masked the independent device-fatal gate.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let cpu = SoftwareEnv::new();
    let channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory.clone(),
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.set_dma_irq_enabled(false)).await?;
    // When: hardware signals a fatal fault and software unmasks that gate.
    let outcome = model
        .interact(|device| {
            device.signal_fatal();
            Ok(StepOutcome::<(), std::convert::Infallible>::Complete(
                Ok(()),
            ))
        })
        .await?;
    assert!(matches!(outcome, Ok(StepOutcome::Complete(Ok(())))));
    cpu.run_main(channel.set_dma_irq_enabled(true)).await?;
    // Then: the sticky fault is visible without replay and device reset permits recovery.
    assert!(!model.irq());
    assert_eq!(
        cpu.run_main(channel.dma_fatal()).await?,
        DmaFatalCode::Internal
    );
    cpu.run_main(channel.reset_dma()).await?;
    let channel = cpu
        .run_main(Channel::setup(
            model,
            memory,
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    assert_eq!(cpu.run_main(channel.dma_fatal()).await?, DmaFatalCode::None);
    Ok(())
}
