#![allow(missing_docs)]

use pdma_model_contract::{
    DeviceEpoch, EpochIrqSource, ModelError, ResourceConfig,
    desc::{CopyCommand, CopyRequest, ExternalCommand},
    ids::{CommandId, CompletionRingId, QueueId},
    irq::IrqSource,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::AsyncDevice;

#[tokio::test(flavor = "current_thread")]
async fn completion_irq_latches_only_after_publication_retry_succeeds() {
    // Given: two copies behind one completion publication slot and an enabled completion IRQ.
    let (runner, worker) = AsyncDevice::queue_port_with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let source = queue.completion_irq_source();
    handle
        .enable_irq_source(source)
        .await
        .expect("completion IRQ source exists");
    handle
        .unmask_irq_source(source)
        .await
        .expect("completion IRQ source exists");
    let src = handle.allocate_buffer(4).await.expect("allocation");
    let first_dst = handle.allocate_buffer(4).await.expect("allocation");
    let second_dst = handle.allocate_buffer(4).await.expect("allocation");
    handle
        .write_buffer(src, 0, &[1, 3, 5, 7])
        .await
        .expect("source write is in bounds");
    for (command_id, dst) in [first_dst, second_dst].into_iter().enumerate() {
        let command = ExternalCommand::Copy(CopyCommand::new(
            CommandId::new(command_id),
            queue.queue_id(),
            queue.completion_ring_id(),
            CopyRequest::new(src, dst, 4),
        ));
        handle
            .submit_command(queue, command)
            .await
            .expect("copy submission has ring space");
    }

    // When: the first completion publishes, the second stalls, and software acks before draining.
    handle.ring_doorbell(queue).await.expect("doorbell rings");
    let first_snapshot = runner.wait_for_progress_after(0).await;
    tokio::task::yield_now().await;
    let stalled_snapshot = runner.snapshot().await;
    let first_irq = handle
        .irq_snapshot()
        .await
        .expect("first published completion asserts IRQ");
    handle
        .ack_irq_source(source)
        .await
        .expect("completion IRQ source exists");
    let retained_irq = handle.irq_snapshot().await;
    let first_completion = handle
        .poll_completion(queue)
        .await
        .expect("completion ring exists")
        .expect("first completion was published");
    let retry_snapshot = runner.wait_for_progress_after(1).await;
    let retry_irq = handle
        .irq_snapshot()
        .await
        .expect("retry publication asserts IRQ");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: early ack preserves the record-backed cause and retry publication stays asserted.
    assert_eq!(first_snapshot.steps, 1);
    assert!(stalled_snapshot.stalls >= 1);
    assert!(first_irq.asserted());
    assert_eq!(first_irq.cause().source(), source.source());
    assert!(retained_irq.is_some_and(pdma_model_contract::IrqSnapshot::asserted));
    assert_eq!(first_completion.command_id(), CommandId::new(0));
    assert_eq!(retry_snapshot.steps, 2);
    assert!(retry_irq.asserted());
    assert_eq!(retry_irq.cause().source(), source.source());
    assert_eq!(summary.steps, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_irq_source_control_returns_error_without_creating_state() {
    // Given: a device with one queue and invalid IRQ source identities.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let unknown_completion = IrqSource::Completion(CompletionRingId::new(99));
    let unknown_event = IrqSource::Event(QueueId::new(99));
    let unknown_completion_cap = EpochIrqSource::new(DeviceEpoch::initial(), unknown_completion);
    let unknown_event_cap = EpochIrqSource::new(DeviceEpoch::initial(), unknown_event);

    // When: software controls unknown IRQ sources.
    let completion_result = handle.enable_irq_source(unknown_completion_cap).await;
    let event_result = handle.mask_irq_source(unknown_event_cap).await;

    // Then: both controls fail and no pending IRQ state is created.
    assert_eq!(
        completion_result,
        Err(ModelError::UnknownIrqSource(unknown_completion))
    );
    assert_eq!(
        event_result,
        Err(ModelError::UnknownIrqSource(unknown_event))
    );
    assert_eq!(handle.irq_snapshot().await, None);
    runner.request_shutdown().await;
    worker.await.expect("runner joins cleanly");
}

#[tokio::test(flavor = "current_thread")]
async fn stale_irq_capability_commits_nothing() {
    // Given: one real IRQ source and a same-source capability from a later device epoch.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            1,
            1,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let source = queue.completion_irq_source();
    handle
        .enable_irq_source(source)
        .await
        .expect("current IRQ capability enables");
    let stale_epoch = DeviceEpoch::initial()
        .next()
        .expect("initial epoch can advance");
    let stale = EpochIrqSource::new(stale_epoch, source.source());
    handle.enable_tracing().await;
    let before = handle.trace_events().await;
    let snapshot_before = handle.irq_snapshot().await;

    // When: every stale IRQ control reaches its device commit boundary.
    let configure = handle
        .configure_irq_route(
            stale,
            pdma_model_contract::IrqRoute::Message(pdma_model_contract::IrqVector::new(7)),
        )
        .await;
    let enable = handle.enable_irq_source(stale).await;
    let disable = handle.disable_irq_source(stale).await;
    let mask = handle.mask_irq_source(stale).await;
    let unmask = handle.unmask_irq_source(stale).await;
    let ack = handle.ack_irq_source(stale).await;

    // Then: every control fails before trace, cause, message, or ack state changes.
    let expected = Err(ModelError::StaleDeviceEpoch {
        expected: DeviceEpoch::initial(),
        actual: stale_epoch,
    });
    assert_ne!(source, stale);
    assert_eq!(configure, expected);
    assert_eq!(enable, expected);
    assert_eq!(disable, expected);
    assert_eq!(mask, expected);
    assert_eq!(unmask, expected);
    assert_eq!(ack, expected);
    assert_eq!(handle.trace_events().await, before);
    assert_eq!(handle.irq_snapshot().await, snapshot_before);
    println!(
        "IRQ epochs current={} stale={} controls={configure:?}/{enable:?}/{disable:?}/{mask:?}/\
         {unmask:?}/{ack:?} trace_unchanged=true snapshot_unchanged=true",
        source.epoch().raw(),
        stale_epoch.raw(),
    );
    runner.request_shutdown().await;
    worker.await.expect("runner joins cleanly");
}

#[tokio::test(flavor = "current_thread")]
async fn event_irq_latches_only_after_publication_retry_succeeds() {
    // Given: two faulty copies behind one event publication slot and an enabled event IRQ.
    let (runner, worker) = AsyncDevice::queue_port_with_resource_config(
        ResourceConfig::default().with_event_publication_slots(1),
    );
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let source = queue.event_irq_source();
    handle
        .enable_irq_source(source)
        .await
        .expect("event IRQ source exists");
    handle
        .unmask_irq_source(source)
        .await
        .expect("event IRQ source exists");
    let src = handle.allocate_buffer(4).await.expect("allocation");
    let dst = handle.allocate_buffer(4).await.expect("allocation");
    for command_id in 0..2 {
        let command = ExternalCommand::Copy(CopyCommand::new(
            CommandId::new(command_id),
            queue.queue_id(),
            queue.completion_ring_id(),
            CopyRequest::new(src, dst, 8),
        ));
        handle
            .submit_command(queue, command)
            .await
            .expect("faulting copy submission has ring space");
    }

    // When: the first event publishes, the second stalls, and software acks before draining.
    handle.ring_doorbell(queue).await.expect("doorbell rings");
    let first_error = runner.wait_for_error_after(0).await;
    tokio::task::yield_now().await;
    let stalled_snapshot = runner.snapshot().await;
    let first_irq = handle.wait_irq().await.expect("first event asserts IRQ");
    handle
        .ack_irq_source(source)
        .await
        .expect("event IRQ source exists");
    let retained_irq = handle.irq_snapshot().await;
    let first_event = handle
        .poll_queue_event(queue)
        .await
        .expect("queue exists")
        .expect("first event was published");
    let retry_error = runner.wait_for_error_after(1).await;
    let retry_irq = handle.wait_irq().await.expect("retry event asserts IRQ");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: early ack preserves the record-backed cause and retry publication stays asserted.
    assert_eq!(first_error.errors, 1);
    assert!(stalled_snapshot.stalls >= 1);
    assert!(first_irq.asserted());
    assert_eq!(first_irq.cause().source(), source.source());
    assert!(retained_irq.is_some_and(pdma_model_contract::IrqSnapshot::asserted));
    assert_eq!(first_event.command_id(), CommandId::new(0));
    assert_eq!(retry_error.errors, 2);
    assert!(retry_irq.asserted());
    assert_eq!(retry_irq.cause().source(), source.source());
    assert_eq!(summary.errors, 2);
}
