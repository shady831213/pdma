//! CPU-owned host interrupt controls and explicit reset recovery.

use hardware_interface_pipeline::{
    AfterEach, AsyncExecutor, AsyncInteractionBackend, Operation, Repeat, StepOutcome, Then,
};
use hardware_interface_runtime::{
    CpuContextError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAccess, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{
    ChannelIrqMaskValues, ChannelRegisters, Command, CompletionFlagsValues, DeviceControl,
    DeviceFaultCode, DeviceRegisters, Endpoint, Response,
};
use pdma_model_device::{
    CheckLengths, Device, NextChunk, Pipeline, ReadCopy, Resettable, UpdateIrq, WriteCopy,
};
use pdma_model_host::{Channel, ChannelConfig, Error, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

mod reset;

async fn with_idle_hardware<F: std::future::Future>(
    model: &Model<SharedMemoryAccess>,
    software: F,
) -> F::Output {
    let mut control = pdma_model_device::ResetControl;
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

fn single_command<M>(device: &Device<M>) -> Pipeline {
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckLengths, Repeat::new(chunk));
    Then::new(
        Then::new(
            device.intake(),
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
        ),
        device.responses(),
    )
}

fn fixture()
-> Result<(Device<SharedMemoryAccess>, SharedMemoryAccess, Rings), Box<dyn std::error::Error>> {
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let device = Device::new(memory.clone(), 1)?;
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
            u32::try_from(DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset())?,
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
    cpu.run_main(channel.set_device_irq_enabled(true)).await?;
    // Then: both independent interrupt gates are enabled.
    for offset in [
        (DeviceRegisters::CHANNELS_OFFSET + pdma_model_contract::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::IRQ_ENABLE.offset(),
        DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset(),
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
async fn control_operations_reject_a_different_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a driver owned by one CPU.
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let channel = owner
        .run_main(Channel::setup(
            model,
            memory,
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    // When: a different CPU attempts every new non-consuming control operation.
    let results = other
        .run_main(async {
            [
                channel.set_irq_mask(ChannelIrqMaskValues::new(3)).await,
                channel.irq_cause().await.map(|_| ()),
                channel.device_fault().await.map(|_| ()),
                channel.set_device_irq_enabled(false).await,
            ]
        })
        .await;
    // Then: each operation rejects the caller at the CPU boundary.
    for result in results {
        assert!(matches!(result, Err(Error::Cpu(CpuContextError::Mismatch))));
    }
    Ok(())
}

#[tokio::test]
async fn device_fault_is_visible_after_unmask_without_replaying_notification()
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
    cpu.run_main(channel.set_device_irq_enabled(false)).await?;
    // When: hardware signals a fatal fault and software unmasks that gate.
    let outcome = model
        .interact(|device| {
            device.signal_fatal();
            StepOutcome::<(), std::convert::Infallible, ()>::Complete(Ok(()))
        })
        .await?;
    assert!(matches!(outcome, StepOutcome::Complete(Ok(()))));
    cpu.run_main(channel.set_device_irq_enabled(true)).await?;
    // Then: the sticky fault is visible without replay and device reset permits recovery.
    assert!(!model.irq());
    assert_eq!(
        cpu.run_main(channel.device_fault()).await?,
        DeviceFaultCode::Internal
    );
    with_idle_hardware(&model, cpu.run_main(channel.reset_device())).await?;
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
    assert_eq!(
        cpu.run_main(channel.device_fault()).await?,
        DeviceFaultCode::None
    );
    Ok(())
}
