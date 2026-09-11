//! CPU-owned host interrupt controls and explicit reset recovery.

use hardware_interface_pipeline::{AsyncExecutor, AsyncInteractionBackend, Operation, StepOutcome};
use hardware_interface_runtime::{
    CpuContextError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAccess, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{
    ChannelIrqMaskValues, ChannelRegisters, Command, CompletionFlagsValues, DeviceControl,
    DeviceFaultCode, DeviceRegisters, Endpoint, Response,
};
use pdma_model_device::{Device, pipeline};
use pdma_model_host::{Channel, Error, RingConfig, Rings, runtime::Model};
use std::sync::Arc;

fn fixture()
-> Result<(Model<SharedMemoryAccess>, SharedMemoryAccess, Rings), Box<dyn std::error::Error>> {
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let model = Model::new(Device::new(memory.clone()));
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
    Ok((model, memory, rings))
}

#[tokio::test]
async fn setup_enables_device_and_channel_interrupts() -> Result<(), Box<dyn std::error::Error>> {
    // Given: disabled hardware and a software CPU.
    let (model, memory, rings) = fixture()?;
    let cpu = SoftwareCpu::new();
    // When: software programs the channel.
    let _channel = cpu
        .run_main(Channel::setup(model.clone(), memory, rings))
        .await?;
    // Then: both independent interrupt gates are enabled.
    for offset in [
        DeviceRegisters::CHANNEL.offset() + ChannelRegisters::IRQ_ENABLE.offset(),
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
    let (model, memory, rings) = fixture()?;
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory, rings))
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
        .run(&mut pipeline(), &mut Operation::new(()))
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
    let (model, memory, rings) = fixture()?;
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let channel = owner.run_main(Channel::setup(model, memory, rings)).await?;
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
async fn reset_operations_reject_a_different_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: two independent drivers owned by one CPU.
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let (model, memory, rings) = fixture()?;
    let channel = owner.run_main(Channel::setup(model, memory, rings)).await?;
    let (model, memory, rings) = fixture()?;
    let device = owner.run_main(Channel::setup(model, memory, rings)).await?;
    // When: the other CPU requests either reset scope.
    let results = other
        .run_main(async { [channel.reset().await, device.reset_device().await] })
        .await;
    // Then: neither reset crosses the ownership boundary.
    for result in results {
        assert!(matches!(result, Err(Error::Cpu(CpuContextError::Mismatch))));
    }
    Ok(())
}

#[tokio::test]
async fn channel_reset_allows_fresh_setup_with_the_same_rings()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a configured channel with one published response.
    let (model, memory, rings) = fixture()?;
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
        .await?;
    let command = Command {
        flags: CompletionFlagsValues::default(),
        id: 9,
        source: Endpoint::linear(0, 0),
        destination: Endpoint::linear(0, 0),
    };
    cpu.run_main(channel.submit(command)).await?;
    AsyncExecutor::new(model.clone())
        .run(&mut pipeline(), &mut Operation::new(()))
        .await
        .map_err(|error| format!("{error:?}"))?;
    // When: software resets and constructs a fresh driver over the same storage.
    cpu.run_main(channel.reset()).await?;
    let mut channel = cpu
        .run_main(Channel::setup(model.clone(), memory, rings))
        .await?;
    cpu.run_main(channel.submit(command)).await?;
    AsyncExecutor::new(model)
        .run(&mut pipeline(), &mut Operation::new(()))
        .await
        .map_err(|error| format!("{error:?}"))?;
    // Then: the fresh cursors expose exactly the new completion.
    assert_eq!(
        cpu.run_main(channel.response())
            .await?
            .map(|response| response.id),
        Some(9)
    );
    assert_eq!(cpu.run_main(channel.response()).await?, None);
    Ok(())
}

#[tokio::test]
async fn device_reset_leaves_disabled_hardware_for_fresh_setup()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: enabled device and channel interrupt gates.
    let (model, memory, rings) = fixture()?;
    let cpu = SoftwareCpu::new();
    let channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
        .await?;
    // When: software requests whole-device reset.
    cpu.run_main(channel.reset_device()).await?;
    // Then: both scopes are disabled and a fresh driver sees healthy hardware.
    for offset in [
        DeviceRegisters::CHANNEL.offset() + ChannelRegisters::ENABLE.offset(),
        DeviceRegisters::CHANNEL.offset() + ChannelRegisters::IRQ_ENABLE.offset(),
        DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset(),
    ] {
        let mut bytes = [0; 4];
        model.read(u32::try_from(offset)?, &mut bytes).await?;
        assert_eq!(u32::from_le_bytes(bytes), 0);
    }
    let channel = cpu.run_main(Channel::setup(model, memory, rings)).await?;
    assert_eq!(
        cpu.run_main(channel.device_fault()).await?,
        DeviceFaultCode::None
    );
    Ok(())
}

#[tokio::test]
async fn device_fault_is_visible_after_unmask_without_replaying_notification()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: software has masked the independent device-fatal gate.
    let (model, memory, rings) = fixture()?;
    let cpu = SoftwareCpu::new();
    let channel = cpu
        .run_main(Channel::setup(model.clone(), memory.clone(), rings))
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
    cpu.run_main(channel.reset_device()).await?;
    let channel = cpu.run_main(Channel::setup(model, memory, rings)).await?;
    assert_eq!(
        cpu.run_main(channel.device_fault()).await?,
        DeviceFaultCode::None
    );
    Ok(())
}
