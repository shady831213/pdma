use pdma_model_contract::semantic::queue::{EventRouting, QueueConfig, SoftwareOwnerId};
use pdma_model_contract::semantic::{DeviceEpoch, EpochIrqSource};
use pdma_model_device::{AsyncDevice, ModelError};

#[tokio::test(flavor = "current_thread")]
async fn stale_irq_capability_commits_nothing() {
    // Given: one real IRQ source and a same-source capability from a later device epoch.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
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
            pdma_model_contract::semantic::IrqRoute::Message(
                pdma_model_contract::semantic::IrqVector::new(7),
            ),
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
