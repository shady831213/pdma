//! Host interrupt controls and explicit reset recovery.

use hardware_interface_pipeline::{AsyncExecutor, AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{
    ChannelIrqMaskValues, ChannelRegisters, Command, CompletionFlagsValues, DmaControl,
    DmaFaultCode, Endpoint, Response,
};
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

mod capabilities;
#[doc = "Test hardware composition."]
pub mod hardware;
mod reset;
use hardware::single_command;

async fn with_idle_hardware<F: std::future::Future>(
    model: &Model<SharedMemoryAccess>,
    software: F,
) -> F::Output {
    let mut control = pdma_model_dma::ResetControl;
    let mut operation = Operation::new(());
    tokio::pin!(software);
    loop {
        tokio::select! {
            result = &mut software => return result,
            result = model.interact(|device| {
                assert!(matches!(operation.step(&mut control, device), StepOutcome::Progressed));
                StepOutcome::<(), std::convert::Infallible, ()>::Progressed
            }) => {
                let Ok(outcome) = result;
                assert!(matches!(outcome, StepOutcome::Progressed));
            }
        }
    }
}

fn fixture()
-> Result<(Dma<SharedMemoryAccess>, SharedMemoryAccess, Rings), Box<dyn std::error::Error>> {
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
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
        response: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Response::RECORD_SIZE)?,
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
    let cpu = SoftwareCpu::new();
    // When: software configures a channel.
    let _channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    // Then: channel setup preserves the device-wide gate.
    let mut bytes = [0; 4];
    model
        .read(
            u32::try_from(
                (pdma_model_contract::DmaDiscovery::EXTENT
                    + pdma_model_contract::DmaAdmin::CONTROL.offset())
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
    let cpu = SoftwareCpu::new();
    // When: software programs the channel.
    let channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.set_dma_irq_enabled(true)).await?;
    // Then: both independent interrupt gates are enabled.
    for offset in [
        (pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::IRQ_ENABLE.offset(),
        (pdma_model_contract::DmaDiscovery::EXTENT
            + pdma_model_contract::DmaAdmin::CONTROL.offset())
            + DmaControl::IRQ_ENABLE.offset(),
    ] {
        let mut bytes = [0; 4];
        model.read(u32::try_from(offset)?, &mut bytes).await?;
        assert_eq!(u32::from_le_bytes(bytes), 1);
    }
    Ok(())
}

#[tokio::test]
async fn masked_completion_causes_remain_visible_without_unmask_catch_up()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both routine completion events are masked.
    let (device, memory, rings) = fixture()?;
    let mut stage = single_command(&device);
    let model = Model::new(device);
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.set_irq_mask(ChannelIrqMaskValues::new(3)))
        .await?;
    cpu.run_main(channel.submit(Command {
        flags: CompletionFlagsValues { ioc: true },
        id: 7,
        source: Endpoint::linear(0, 0),
        destination: Endpoint::linear(0, 0),
    }))
    .await?;
    // When: hardware publishes a masked IOC completion and software unmasks.
    AsyncExecutor::new(model.clone())
        .run(&mut stage, &mut Operation::new(()))
        .await
        .map_err(|error| format!("{error:?}"))?;
    cpu.run_main(channel.set_irq_mask(ChannelIrqMaskValues::default()))
        .await?;
    // Then: software can inspect both causes and consume the response without a catch-up IRQ.
    assert!(!model.irq());
    let cause = cpu.run_main(channel.irq_cause()).await?;
    assert!(cause.response_non_empty());
    assert!(cause.ioc());
    assert_eq!(
        cpu.run_main(channel.response())
            .await?
            .map(|response| response.id),
        Some(7)
    );
    Ok(())
}

#[tokio::test]
async fn dma_fault_is_visible_after_unmask_without_replaying_notification()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: software has masked the independent device-fatal gate.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let cpu = SoftwareCpu::new();
    let channel = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory.clone(),
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    cpu.run_main(channel.set_dma_irq_enabled(false)).await?;
    // When: hardware signals a fatal fault and software unmasks that gate.
    let outcome = model
        .interact(|device| {
            device.signal_fatal();
            StepOutcome::<(), std::convert::Infallible, ()>::Complete(Ok(()))
        })
        .await?;
    assert!(matches!(outcome, StepOutcome::Complete(Ok(()))));
    cpu.run_main(channel.set_dma_irq_enabled(true)).await?;
    // Then: the sticky fault is visible without replay and device reset permits recovery.
    assert!(!model.irq());
    assert_eq!(
        cpu.run_main(channel.dma_fault()).await?,
        DmaFaultCode::Internal
    );
    with_idle_hardware(&model, cpu.run_main(channel.reset_dma())).await?;
    let channel = cpu
        .run_main(Channel::setup(
            model,
            memory,
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    assert_eq!(cpu.run_main(channel.dma_fault()).await?, DmaFaultCode::None);
    Ok(())
}
