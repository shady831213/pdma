use super::{Access, RecordingTransport, activation, expected_queue_transcript};
use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
use pdma_model_contract::{CompletionRingId, EpochIrqSource, IrqSource, IrqVector, QueueId};
use pdma_model_contract::{DeviceEpoch, EpochQueue, QueueCursorUpdate, RingCursor};
use pdma_model_device::{AsyncDevice, MmioBatchError};
use pdma_model_host::{
    AsyncPdmaBus, BusDmaStage, BusDmaWrite, BusSubmission, MmioDeviceBus, QueueControlWrite,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test(flavor = "current_thread")]
async fn queue_recording_pins_literal_access_order_and_typed_observation() -> TestResult {
    // Given: one literal recording transport and a complete queue/observe program.
    let transport = RecordingTransport::new();
    let bus = MmioDeviceBus::new(transport.clone());
    let stage = bus.stage_dma_regions(&[128, 64, 128]).await?;
    let activation = activation(stage.proposed_regions())?;
    let queue = activation.queue();

    // When: both closed batches, every queue command, and OBSERVE_SOURCE cross MMIO.
    bus.bootstrap_queue(stage, activation).await?;
    bus.accept_submission(BusSubmission::new(
        queue,
        RingCursor::new(0),
        BusDmaWrite::new(
            activation.rings().submission().region().base(),
            vec![0x5a; 64],
        ),
        None,
    ))
    .await?;
    bus.read_queue_status(queue).await?;
    let writes = [
        QueueControlWrite::CaptureSubmissionProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControlWrite::PublishSubmissionConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControlWrite::PublishCompletionProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControlWrite::PublishCompletionConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControlWrite::PublishEventProducer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControlWrite::PublishEventConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
    ];
    for request in writes {
        bus.write_queue_control(request).await?;
    }
    let source = EpochIrqSource::new(
        DeviceEpoch::initial(),
        IrqSource::Completion(CompletionRingId::new(0)),
    );
    let observed = bus.observe_irq_source(source).await?;

    // Then: literal offsets, LE values, batch boundaries, and typed state are exact.
    assert_eq!(
        observed,
        IrqSourceStatus::try_from(0x13).map_err(|_| "valid literal IRQ state")?
    );
    let mut expected = vec![Access::BootstrapBatch, Access::SubmissionBatch];
    expected.extend(expected_queue_transcript());
    expected.extend([
        Access::Write(0x200, 0_u64.to_le_bytes().to_vec()),
        Access::Write(0x208, 0_u64.to_le_bytes().to_vec()),
        Access::Write(0x210, 0_u32.to_le_bytes().to_vec()),
        Access::Write(0x220, 7_u32.to_le_bytes().to_vec()),
        Access::Read(0x224, 4),
        Access::Read(0x254, 4),
    ]);
    assert_eq!(transport.accesses(), expected);
    println!("RECORDED_ACCESSES={}", transport.accesses_json());
    assert_eq!(
        queue,
        EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0))
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn concrete_adapters_return_identical_typed_irq_status() -> TestResult {
    // Given: one real device bootstrapped only through the MMIO bus batch.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let bus = MmioDeviceBus::new(handle.clone());
    let stage = bus.stage_dma_regions(&[128, 64, 128]).await?;
    let activation = activation(stage.proposed_regions())?;
    bus.bootstrap_queue(stage, activation).await?;
    let source = EpochIrqSource::new(
        DeviceEpoch::initial(),
        IrqSource::Completion(CompletionRingId::new(0)),
    );

    // When: typed and MMIO adapters observe the same source core.
    let typed = AsyncPdmaBus::observe_irq_source(&handle, source).await?;
    let mmio = AsyncPdmaBus::observe_irq_source(&bus, source).await?;

    // Then: the typed status is identical without an MMIO typed-control escape.
    assert_eq!(typed, mmio);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn selector_lock_is_clone_shared_and_released_on_cancellation() -> TestResult {
    // Given: one bootstrapped recording bus with its next queue staging write gated.
    let transport = RecordingTransport::new();
    let bus = MmioDeviceBus::new(transport.clone());
    let stage = bus.stage_dma_regions(&[128, 64, 128]).await?;
    let activation = activation(stage.proposed_regions())?;
    let queue = activation.queue();
    bus.bootstrap_queue(stage, activation).await?;
    transport.clear();
    transport.gate_next_queue_start();

    // When: one clone is cancelled while holding the selector and another proceeds.
    let first_bus = bus.clone();
    let first = tokio::spawn(async move { first_bus.read_queue_status(queue).await });
    transport.wait_queue_start().await;
    let second_bus = bus.clone();
    let second = tokio::spawn(async move { second_bus.read_queue_status(queue).await });
    tokio::task::yield_now().await;
    assert_eq!(
        transport.accesses(),
        vec![Access::Write(0x100, 0_u64.to_le_bytes().to_vec())]
    );
    println!("SELECTOR_CONFLICT={:?}", transport.accesses());
    first.abort();
    let first_result = first.await;
    let second_result = second.await?;

    // Then: cancellation releases the lock and no second client stages before release.
    assert!(first_result.is_err_and(|error| error.is_cancelled()));
    assert!(second_result.is_ok());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn injected_access_result_and_batch_failures_are_exact_and_release_locks() -> TestResult {
    // Given: independent transports configured for one failure at each boundary.
    let batch_transport = RecordingTransport::new();
    batch_transport.fail_next_batch();
    let batch_bus = MmioDeviceBus::new(batch_transport.clone());
    let batch_stage = batch_bus.stage_dma_regions(&[128, 64, 128]).await?;
    let batch_activation = activation(batch_stage.proposed_regions())?;
    let access_transport = RecordingTransport::new();
    let access_bus = MmioDeviceBus::new(access_transport.clone());
    let access_stage = access_bus.stage_dma_regions(&[128, 64, 128]).await?;
    let access_activation = activation(access_stage.proposed_regions())?;
    let queue = access_activation.queue();
    access_bus
        .bootstrap_queue(access_stage, access_activation)
        .await?;

    // When: batch, access, and semantic result failures are injected deterministically.
    let batch = batch_bus
        .bootstrap_queue(batch_stage, batch_activation)
        .await;
    access_transport.fail_next_write(0x100);
    let access = access_bus.read_queue_status(queue).await;
    access_transport.set_queue_result(0x103);
    let result = access_bus.read_queue_status(queue).await;
    access_transport.set_queue_result(0xdead);
    let malformed = access_bus.read_queue_status(queue).await;
    access_transport.set_queue_result(1);
    let recovered = access_bus.read_queue_status(queue).await;

    // Then: exact typed failures surface and every selector lock remains usable.
    assert_eq!(
        batch,
        Err(pdma_model_host::MmioBusControlError::Batch(
            MmioBatchError::InvalidShape
        ))
    );
    assert_eq!(
        access,
        Err(pdma_model_host::MmioBusControlError::Access(
            pdma_model_contract::abi::mmio::MmioAccessError::UnmappedOffset { offset: 0x100 }
        ))
    );
    assert_eq!(
        result,
        Err(pdma_model_host::MmioBusControlError::Result(
            MmioResultDetails {
                result: MmioResult::InvalidQueue,
                details: [0; 3],
            }
        ))
    );
    assert_eq!(
        malformed,
        Err(pdma_model_host::MmioBusControlError::MalformedResponse)
    );
    assert!(recovered.is_ok());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn notification_wait_claims_once_through_the_raw_irq_window() -> TestResult {
    // Given: readiness exposes one Message notification without consuming it.
    let transport = RecordingTransport::new();
    transport.set_message_notification(41, 7);
    let bus = MmioDeviceBus::new(transport.clone());

    // When: the MMIO bus waits for and claims the notification.
    let notification = bus
        .wait_irq_notification()
        .await
        .ok_or("expected notification")?;

    // Then: one raw claim, result read, and exact response reads produced that identity.
    assert_eq!(notification.vector().raw(), 41);
    assert_eq!(
        notification.source(),
        IrqSource::Completion(CompletionRingId::new(0))
    );
    assert_eq!(
        transport.accesses(),
        vec![
            Access::Write(0x220, 9_u32.to_le_bytes().to_vec()),
            Access::Read(0x224, 4),
            Access::Read(0x240, 8),
            Access::Read(0x248, 8),
            Access::Read(0x250, 4),
            Access::Read(0x254, 4),
            Access::Read(0x258, 4),
            Access::Read(0x25c, 4),
            Access::Read(0x260, 8),
            Access::Read(0x268, 8),
            Access::Read(0x270, 8),
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn notification_wait_retries_empty_race_and_shutdown_never_claims() -> TestResult {
    // Given: one readiness wake races with a claimant before the bus acquires the window.
    let transport = RecordingTransport::new();
    transport.set_message_notification(55, 3);
    transport.empty_next_claim();
    let bus = MmioDeviceBus::new(transport.clone());

    // When: the first claim reports EMPTY and the next readiness turn claims the message.
    let notification = bus
        .wait_irq_notification()
        .await
        .ok_or("expected raced notification")?;
    let accesses = transport.accesses();
    transport.clear();
    let shutdown = bus.wait_irq_notification().await;

    // Then: the selector was released between two claims and shutdown issued no third claim.
    assert_eq!(notification.vector(), IrqVector::new(55));
    assert_eq!(
        accesses.get(..4),
        Some(
            &[
                Access::Write(0x220, 9_u32.to_le_bytes().to_vec()),
                Access::Read(0x224, 4),
                Access::Write(0x220, 9_u32.to_le_bytes().to_vec()),
                Access::Read(0x224, 4),
            ][..]
        )
    );
    assert_eq!(shutdown, None);
    assert!(transport.accesses().is_empty());
    Ok(())
}
