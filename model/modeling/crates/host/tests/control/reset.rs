use super::{
    AsyncExecutor, Channel, ChannelConfig, ChannelRegisters, Command, CompletionFlagsValues,
    CpuContextError, DeviceControl, DeviceFaultCode, DeviceRegisters, Endpoint, Error, Model,
    Operation, RegisterAccess, SoftwareCpu, fixture, single_command, with_idle_hardware,
};

#[tokio::test]
async fn reset_operations_reject_a_different_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: two independent drivers owned by one CPU.
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
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
    let (device, memory, rings) = fixture()?;
    let model = Model::new(device);
    let device = owner
        .run_main(Channel::setup(
            model,
            memory,
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
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
    let (device, memory, rings) = fixture()?;
    let mut stage = pdma_model_device::RunCommand::new(single_command(&device));
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
    let command = Command {
        flags: CompletionFlagsValues::default(),
        id: 9,
        source: Endpoint::linear(0, 0),
        destination: Endpoint::linear(0, 0),
    };
    cpu.run_main(channel.submit(command)).await?;
    assert_eq!(
        AsyncExecutor::new(model.clone())
            .run(&mut stage, &mut Operation::new(()))
            .await
            .map_err(|error| format!("{error:?}"))?,
        std::ops::ControlFlow::Continue(())
    );
    // When: software resets and constructs a fresh driver over the same storage.
    with_idle_hardware(&model, cpu.run_main(channel.reset())).await?;
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
    cpu.run_main(channel.submit(command)).await?;
    assert_eq!(
        AsyncExecutor::new(model)
            .run(&mut stage, &mut Operation::new(()))
            .await
            .map_err(|error| format!("{error:?}"))?,
        std::ops::ControlFlow::Continue(())
    );
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
    // When: software requests whole-device reset.
    cpu.run_main(channel.set_device_irq_enabled(true)).await?;
    with_idle_hardware(&model, cpu.run_main(channel.reset_device())).await?;
    // Then: both scopes are disabled and a fresh driver sees healthy hardware.
    for offset in [
        (DeviceRegisters::CHANNELS_OFFSET + pdma_model_contract::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::ENABLE.offset(),
        (DeviceRegisters::CHANNELS_OFFSET + pdma_model_contract::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::IRQ_ENABLE.offset(),
        DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset(),
    ] {
        let mut bytes = [0; 4];
        model.read(u32::try_from(offset)?, &mut bytes).await?;
        assert_eq!(u32::from_le_bytes(bytes), 0);
    }
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
