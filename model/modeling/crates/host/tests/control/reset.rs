use pdma_model_contract::dma::{
    ChannelConfiguration, ChannelInterrupts, CopyCommandDestinationValues, CopyCommandSourceValues,
    LinearEndpoint,
};

use super::{
    AsyncExecutor, Channel, ChannelConfig, ChannelRegisters, Command, DmaControl, DmaFatalCode,
    Model, RegisterAccess, SoftwareCpu, fixture,
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
                    id: pdma_model_contract::dma::ChannelId::new(0),
                    rings,
                },
            ))
            .await?;
        // When: another CPU requests reset and waits for hardware cleanup.
        other
            .run_main(async {
                if whole_dma {
                    channel.reset_dma().await
                } else {
                    channel.reset().await
                }
            })
            .await?;
        // Then: both reset scopes leave disabled hardware ready for fresh configuration.
        let mut enable = [0; 4];
        model
            .read(
                u32::try_from(
                    pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                        + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                        + ChannelRegisters::CONFIG.offset()
                        + ChannelConfiguration::ENABLE.offset(),
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
    // Given: a configured channel with one published event.
    let (device, memory, rings) = fixture()?;
    let mut hardware = device.hardware();
    let model = Model::new(device);
    let executor = AsyncExecutor::new(model.clone());
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    let software = async {
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
        let command = Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 9,
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
        });
        cpu.run_main(channel.submit(command)).await?;
        let producer = u32::try_from(
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
                + pdma_model_contract::dma::EventRegisters::PRODUCER.offset(),
        )?;
        let mut published = false;
        for _ in 0..256 {
            let mut bytes = [0; 4];
            model.read(producer, &mut bytes).await?;
            if u32::from_le_bytes(bytes) == 1 {
                published = true;
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(published);
        // When: software resets and constructs a fresh driver over the same storage.
        cpu.run_main(channel.reset()).await?;
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
        cpu.run_main(channel.submit(command)).await?;
        let mut completed = None;
        for _ in 0..256 {
            completed = cpu.run_main(channel.event()).await?;
            if completed.is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
        // Then: the fresh cursors expose exactly the new completion.
        assert_eq!(completed.map(|event| event.id), Some(9));
        assert_eq!(cpu.run_main(channel.event()).await?, None);
        Ok::<_, Box<dyn std::error::Error>>(())
    };
    tokio::select! {
        result = software => result,
        result = executor.drive(&mut hardware, &mut operation) => Err(format!("hardware terminated: {result:?}").into()),
    }
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
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    // When: software requests whole-device reset.
    cpu.run_main(channel.set_dma_irq_enabled(true)).await?;
    cpu.run_main(channel.reset_dma()).await?;
    // Then: both scopes are disabled and a fresh driver sees healthy hardware.
    for offset in [
        (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::CONTROL.offset())
            + ChannelRegisters::CONFIG.offset()
            + ChannelConfiguration::ENABLE.offset(),
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
        assert_eq!(u32::from_le_bytes(bytes), 0);
    }
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
