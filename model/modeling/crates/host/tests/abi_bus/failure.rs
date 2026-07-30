struct ActiveProbe {
    bus: AsyncDeviceHandle,
    proposed: [DmaRegion; 3],
    queue: EpochQueue,
}

async fn real_bus_happy_probe() -> Result<(), Box<dyn std::error::Error>> {
    let (device, _worker) = AsyncDevice::queue_port();
    let bus = device.handle();
    let before_allocation = bus.snapshot().await;
    let region = AsyncPdmaBus::allocate_dma_region(&bus, 8).await?;
    let after_allocation = bus.wait_for_change_after(before_allocation).await;
    assert!(after_allocation.generation() > before_allocation.generation());

    let before_write = bus.snapshot().await;
    AsyncPdmaBus::write_dma(&bus, region.base(), &[2, 4, 6, 8]).await?;
    let after_write = bus.wait_for_change_after(before_write).await;
    assert!(after_write.generation() > before_write.generation());
    assert_eq!(
        AsyncPdmaBus::read_dma(&bus, region.base(), 4).await?,
        vec![2, 4, 6, 8]
    );

    let rings = AsyncPdmaBus::stage_dma_regions(&bus, &[128, 64, 128]).await?;
    let proposed: [DmaRegion; 3] = rings
        .proposed_regions()
        .try_into()
        .map_err(|_| "real bus must propose three ring regions")?;
    let activated = AsyncPdmaBus::bootstrap_queue(
        &bus,
        rings,
        activation(&proposed, QueueId::new(0))?,
    )
    .await?;
    let queue = activated.queue();
    assert_eq!(AsyncPdmaBus::read_queue_status(&bus, queue).await?, activated);
    assert_eq!(
        AsyncPdmaBus::write_queue_control(
            &bus,
            QueueControlWrite::CaptureSubmissionProducer(QueueCursorUpdate::new(
                queue,
                RingCursor::new(0),
            )),
        )
        .await?,
        activated
    );
    Ok(())
}

async fn real_bus_failure_probe() -> Result<(), Box<dyn std::error::Error>> {
    let active = probe_dropped_and_conflicting_bootstrap().await?;
    let (bus, winner) = probe_failed_dma_and_control(active).await?;
    probe_pending_irq_and_stale_epoch(&bus, winner).await
}

async fn probe_dropped_and_conflicting_bootstrap() -> Result<ActiveProbe, Box<dyn std::error::Error>>
{
    let (device, _worker) = AsyncDevice::queue_port();
    let bus = device.handle();
    let initial = bus.snapshot().await;

    drop(AsyncPdmaBus::allocate_dma_region(&bus, 16));
    let dropped_stage = AsyncPdmaBus::stage_dma_regions(&bus, &[128, 64, 128]);
    drop(dropped_stage);
    drop(AsyncPdmaBus::write_dma(
        &bus,
        DmaAddress::new(0x1_0000),
        &[0x11],
    ));
    assert_eq!(bus.dma_generation().await, 0);
    assert_eq!(bus.region_for_object(BufferId::new(0)).await, None);
    assert_eq!(bus.snapshot().await.generation(), initial.generation());

    let rings = AsyncPdmaBus::stage_dma_regions(&bus, &[128, 64, 128]).await?;
    let proposed: [DmaRegion; 3] = rings
        .proposed_regions()
        .try_into()
        .map_err(|_| "raw bus must propose three ring regions")?;
    let wrong = activation(&proposed, QueueId::new(1))?;
    assert_eq!(
        AsyncPdmaBus::bootstrap_queue(&bus, rings.clone(), wrong).await,
        Err(BusControlError::InvalidQueue)
    );
    assert_eq!(bus.dma_generation().await, 0);

    let valid = activation(&proposed, QueueId::new(0))?;
    let before_bootstrap = bus.snapshot().await;
    let status = AsyncPdmaBus::bootstrap_queue(&bus, rings, valid).await?;
    let after_bootstrap = bus.wait_for_change_after(before_bootstrap).await;
    assert!(after_bootstrap.generation() > before_bootstrap.generation());
    assert_eq!(bus.dma_generation().await, 1);
    Ok(ActiveProbe {
        bus,
        proposed,
        queue: status.queue(),
    })
}

async fn probe_failed_dma_and_control(
    active: ActiveProbe,
) -> Result<(AsyncDeviceHandle, DmaRegion), Box<dyn std::error::Error>> {
    let ActiveProbe {
        bus,
        proposed: [submission, _, _],
        queue,
    } = active;
    let stale_indirect = AsyncPdmaBus::stage_dma_regions(&bus, &[8]).await?;
    let [indirect_region] = stale_indirect.proposed_regions() else {
        return Err("raw bus must propose one indirect region".into());
    };
    let indirect_region = *indirect_region;
    let winner = AsyncPdmaBus::allocate_dma_region(&bus, 4).await?;
    let stale = BusSubmission::new(
        queue,
        RingCursor::new(0),
        BusDmaWrite::new(submission.base(), vec![0x33; 64]),
        Some((
            stale_indirect,
            BusDmaWrite::new(indirect_region.base(), vec![0x44; 8]),
        )),
    );
    assert_eq!(
        AsyncPdmaBus::accept_submission(&bus, stale).await,
        Err(BusControlError::InvalidTransaction)
    );
    assert_eq!(
        AsyncPdmaBus::read_dma(&bus, submission.base(), 64).await?,
        vec![0; 64]
    );
    assert_eq!(
        bus.region_for_object(indirect_region.object()).await,
        Some(winner)
    );
    assert_ne!(winner, indirect_region);
    assert_eq!(
        AsyncPdmaBus::read_queue_status(&bus, queue)
            .await?
            .submission()
            .producer(),
        RingCursor::new(0)
    );

    let before_failed_writes = bus.snapshot().await;
    assert_eq!(
        AsyncPdmaBus::write_dma(&bus, DmaAddress::new(0xffff_0000), &[1]).await,
        Err(DmaSpaceError::Unmapped)
    );
    assert_eq!(bus.snapshot().await, before_failed_writes);
    assert_eq!(
        AsyncPdmaBus::write_dma(&bus, winner.base(), &[1, 2, 3, 4, 5]).await,
        Err(DmaSpaceError::CrossesRegion)
    );
    assert_eq!(bus.snapshot().await, before_failed_writes);
    assert_eq!(
        AsyncPdmaBus::read_dma(&bus, winner.base(), winner.length()).await?,
        vec![0; 4]
    );

    let conflicting = BusSubmission::new(
        queue,
        RingCursor::new(1),
        BusDmaWrite::new(submission.base(), vec![0x55; 64]),
        None,
    );
    assert_eq!(
        AsyncPdmaBus::accept_submission(&bus, conflicting).await,
        Err(BusControlError::InvalidTransaction)
    );
    assert_eq!(
        AsyncPdmaBus::write_queue_control(
            &bus,
            QueueControlWrite::PublishCompletionConsumer(QueueCursorUpdate::new(
                queue,
                RingCursor::new(1),
            )),
        )
        .await,
        Err(BusControlError::Abi(
            pdma_model_contract::abi::AbiControlError::CursorRegression {
                producer: 0,
                consumer: 1,
            }
        ))
    );
    Ok((bus, winner))
}

async fn probe_pending_irq_and_stale_epoch(
    bus: &AsyncDeviceHandle,
    winner: DmaRegion,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pending_irq = Box::pin(AsyncPdmaBus::wait_irq_notification(bus));
    for _ in 0..2 {
        tokio::select! {
            biased;
            notification = &mut pending_irq => {
                return Err(format!("unexpected IRQ notification: {notification:?}").into());
            }
            () = async {} => {}
        }
    }
    drop(pending_irq);

    let next_epoch = bus
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    assert_eq!(next_epoch, DeviceEpoch::new(1));
    assert_eq!(
        AsyncPdmaBus::read_queue_status(
            bus,
            EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0)),
        )
        .await,
        Err(BusControlError::StaleEpoch)
    );
    assert_eq!(bus.dma_generation().await, 2);
    assert_eq!(
        AsyncPdmaBus::read_dma(bus, winner.base(), winner.length()).await?,
        vec![0; 4]
    );
    Ok(())
}
