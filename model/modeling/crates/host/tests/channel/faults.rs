use super::*;

#[tokio::test]
async fn fault_interrupt_arrives_when_event_write_is_unmapped()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the event ring points outside the injected memory backend.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
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
        event: RingConfig::new(0x8000, 1)?,
    };
    let destination = allocator.allocate(8, alignment)?;
    let memory = allocator.memory_access();
    let device = Dma::new(memory.clone(), 1)?;
    let mut hardware = device.hardware();
    let mut model = Model::new(device);
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
    cpu.run_main(
        channel.submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 9,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0,
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

    // When: event publication fails at the real memory boundary.
    until_irq(model.clone(), &mut hardware).await?;

    // Then: the notification is claimed once while the fatal remains visible through masking.
    model.wait_ready().await?;
    assert!(model.claim(()).await?.is_some());
    assert_eq!(
        cpu.run_main(channel.fatal()).await?,
        ChannelFatalCode::EventWrite
    );
    assert_eq!(cpu.run_main(channel.event()).await?, None);
    cpu.run_main(channel.set_irq_enabled(false)).await?;
    assert!(model.claim(()).await?.is_none());
    assert!(!model.irq());
    assert_eq!(
        cpu.run_main(channel.fatal()).await?,
        ChannelFatalCode::EventWrite
    );
    Ok(())
}
