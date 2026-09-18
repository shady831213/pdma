use super::{
    AsyncExecutor, Channel, ChannelConfig, ChannelRegisters, Command, CompletionFlagsValues,
    DmaControl, DmaFaultCode, Endpoint, Model, Operation, RegisterAccess, SoftwareCpu, fixture,
    single_command, with_idle_hardware,
};

#[tokio::test]
async fn reset_operations_can_run_on_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    for whole_dma in [false, true] {
        // Given: a driver created on one CPU and independently driven reset hardware.
        let owner = SoftwareCpu::new();
        let other = SoftwareCpu::new();
        let (device, memory, rings) = fixture()?;
        let model = Model::new(device);
        let channel = owner
            .run_main(Channel::setup(
                model.clone(),
                memory,
                ChannelConfig {
                    id: pdma_model_contract::ChannelId::new(0),
                    rings,
                },
            ))
            .await?;
        // When: another CPU requests reset and waits for hardware cleanup.
        with_idle_hardware(
            &model,
            other.run_main(async {
                if whole_dma {
                    channel.reset_dma().await
                } else {
                    channel.reset().await
                }
            }),
        )
        .await?;
        // Then: both reset scopes leave disabled hardware ready for fresh configuration.
        let mut enable = [0; 4];
        model
            .read(
                u32::try_from(
                    pdma_model_contract::ChannelBlock::ALIGNMENT
                        + pdma_model_contract::ChannelBlock::CONTROL.offset()
                        + ChannelRegisters::ENABLE.offset(),
                )?,
                &mut enable,
            )
            .await?;
        assert_eq!(u32::from_le_bytes(enable), 0);
    }
    Ok(())
}

#[tokio::test]
async fn channel_reset_allows_fresh_setup_with_the_same_rings()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a configured channel with one published response.
    let (device, memory, rings) = fixture()?;
    let mut stage = pdma_model_dma::RunCommand::new(single_command(&device));
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
    cpu.run_main(channel.set_dma_irq_enabled(true)).await?;
    with_idle_hardware(&model, cpu.run_main(channel.reset_dma())).await?;
    // Then: both scopes are disabled and a fresh driver sees healthy hardware.
    for offset in [
        (pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::ENABLE.offset(),
        (pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::IRQ_ENABLE.offset(),
        (pdma_model_contract::DmaDiscovery::EXTENT
            + pdma_model_contract::DmaAdmin::CONTROL.offset())
            + DmaControl::IRQ_ENABLE.offset(),
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
    assert_eq!(cpu.run_main(channel.dma_fault()).await?, DmaFaultCode::None);
    Ok(())
}
