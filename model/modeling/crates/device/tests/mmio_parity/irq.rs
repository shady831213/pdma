use super::{TestResult, read_u32, read_u64, write_u32, write_u64};
use pdma_model_contract::abi::mmio::{IrqSourceStatus, MmioResult};
use pdma_model_contract::semantic::{
    Command, CommandId, CopyRequest, EventRouting, QueueConfig, QueueConfigHandle, SoftwareOwnerId,
    irq::{IrqNotification, IrqRoute, IrqVector},
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};

#[tokio::test(flavor = "current_thread")]
async fn readiness_preserves_one_message_until_one_raw_claim() -> TestResult {
    // Given: one masked Message-routed completion source with one pending notification.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let queue = create_queue(&handle).await?;
    select_completion(&handle, queue).await?;
    write_u32(&handle, 0x214, 1).await?;
    write_u64(&handle, 0x218, 41).await?;
    write_u32(&handle, 0x220, 1).await?;
    write_u32(&handle, 0x220, 2).await?;
    submit_copy(&handle, queue).await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;

    // When: raw unmask wakes a readiness waiter and two readiness checks precede one claim.
    let first_readiness = tokio::spawn({
        let handle = handle.clone();
        async move { handle.wait_irq_notification_ready().await }
    });
    let second_readiness = tokio::spawn({
        let handle = handle.clone();
        async move { handle.wait_irq_notification_ready().await }
    });
    tokio::task::yield_now().await;
    assert!(!first_readiness.is_finished());
    assert!(!second_readiness.is_finished());
    write_u32(&handle, 0x220, 5).await?;
    assert!(first_readiness.await?);
    assert!(second_readiness.await?);
    assert!(handle.irq_notification_ready().await);
    write_u32(&handle, 0x220, 9).await?;
    let first = [
        read_u64(&handle, 0x240).await?,
        read_u64(&handle, 0x248).await?,
        u64::from(read_u32(&handle, 0x250).await?),
        u64::from(read_u32(&handle, 0x25c).await?),
        read_u64(&handle, 0x260).await?,
        read_u64(&handle, 0x268).await?,
    ];

    // Then: claim returns the same source/vector/ID once; a second claimant gets EMPTY.
    assert_eq!(read_u32(&handle, 0x224).await?, MmioResult::Ok.code());
    assert_eq!(first, [0, 0, 0, 2, 41, 0]);
    assert!(!handle.irq_notification_ready().await);
    write_u32(&handle, 0x220, 9).await?;
    assert_eq!(read_u32(&handle, 0x224).await?, MmioResult::Empty.code());
    assert_eq!(response(&handle).await?, [0; 9]);
    device.request_shutdown().await;
    assert!(!handle.wait_irq_notification_ready().await);
    worker.await?;
    println!(
        "{{\"ready_checks\":2,\"source\":0,\"vector\":41,\"message_id\":0,\"claims\":1,\"second\":\"EMPTY\"}}"
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn typed_irq_observation_and_notification_match_raw_mmio() -> TestResult {
    // Given: equivalent typed and raw devices with enabled Message-routed completion sources.
    let (typed_device, typed_worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let (raw_device, raw_worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let typed_worker = tokio::spawn(typed_worker.run());
    let raw_worker = tokio::spawn(raw_worker.run());
    let typed = typed_device.handle();
    let raw = raw_device.handle();
    let typed_queue = create_queue(&typed).await?;
    let raw_queue = create_queue(&raw).await?;
    let source = typed_queue.completion_irq_source();
    let vector = IrqVector::new(41);
    typed
        .configure_irq_route(source, IrqRoute::Message(vector))
        .await?;
    typed.enable_irq_source(source).await?;
    typed.unmask_irq_source(source).await?;
    select_completion(&raw, raw_queue).await?;
    write_u32(&raw, 0x214, 1).await?;
    write_u64(&raw, 0x218, u64::try_from(vector.raw())?).await?;
    for command in [1, 2, 5] {
        write_u32(&raw, 0x220, command).await?;
    }

    // When: both public surfaces observe, produce, and consume one notification.
    let typed_before = typed.observe_irq_source(source).await?;
    write_u32(&raw, 0x220, 7).await?;
    let raw_before = IrqSourceStatus::try_from(read_u32(&raw, 0x254).await?)
        .map_err(|_| "valid raw IRQ status")?;
    submit_copy(&typed, typed_queue).await?;
    submit_copy(&raw, raw_queue).await?;
    typed.ring_doorbell(typed_queue).await?;
    raw.ring_doorbell(raw_queue).await?;
    typed_device.wait_for_progress_after(0).await;
    raw_device.wait_for_progress_after(0).await;
    let typed_notification = typed
        .wait_irq_notification()
        .await
        .ok_or("typed notification")?;
    assert!(raw.wait_irq_notification_ready().await);
    write_u32(&raw, 0x220, 9).await?;
    let raw_notification = response(&raw).await?;

    // Then: source state, notification identity, and consuming post-state are identical.
    assert_eq!(typed_before, raw_before);
    let IrqNotification::Message(message) = typed_notification else {
        return Err("expected typed Message notification".into());
    };
    assert_eq!(
        raw_notification[0],
        message.source_capability().epoch().raw()
    );
    assert_eq!(raw_notification[1], 0);
    assert_eq!(raw_notification[2], 0);
    assert_eq!(raw_notification[5], 2);
    assert_eq!(raw_notification[6], u64::try_from(message.vector().raw())?);
    assert_eq!(raw_notification[7], u64::try_from(message.id().raw())?);
    assert_eq!(message.source_capability(), source);
    assert!(!typed.irq_notification_ready().await);
    assert!(!raw.irq_notification_ready().await);
    write_u32(&raw, 0x220, 9).await?;
    assert_eq!(read_u32(&raw, 0x224).await?, MmioResult::Empty.code());
    typed_device.request_shutdown().await;
    raw_device.request_shutdown().await;
    typed_worker.await?;
    raw_worker.await?;
    println!(
        "{{\"typed_observe_bits\":{},\"raw_observe_bits\":{},\"source\":0,\"vector\":41,\"message_id\":0,\"post_ready\":false}}",
        typed_before.bits(),
        raw_before.bits()
    );
    Ok(())
}

async fn create_queue(handle: &AsyncDeviceHandle) -> TestResult<QueueConfigHandle> {
    Ok(handle
        .create_queue(QueueConfig::new(
            1,
            1,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await?)
}

async fn select_completion(handle: &AsyncDeviceHandle, queue: QueueConfigHandle) -> TestResult {
    write_u64(handle, 0x200, queue.epoch().raw()).await?;
    write_u64(
        handle,
        0x208,
        u64::try_from(queue.completion_ring_id().raw())?,
    )
    .await?;
    write_u32(handle, 0x210, 0).await
}

async fn submit_copy(handle: &AsyncDeviceHandle, queue: QueueConfigHandle) -> TestResult {
    let source = handle.allocate_buffer(1).await?;
    let destination = handle.allocate_buffer(1).await?;
    handle
        .submit_command(
            queue,
            Command::new(
                CommandId::new(0),
                pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                    source,
                    destination,
                    1,
                )),
            ),
        )
        .await?;
    Ok(())
}

async fn response(handle: &AsyncDeviceHandle) -> TestResult<[u64; 9]> {
    Ok([
        read_u64(handle, 0x240).await?,
        read_u64(handle, 0x248).await?,
        u64::from(read_u32(handle, 0x250).await?),
        u64::from(read_u32(handle, 0x254).await?),
        u64::from(read_u32(handle, 0x258).await?),
        u64::from(read_u32(handle, 0x25c).await?),
        read_u64(handle, 0x260).await?,
        read_u64(handle, 0x268).await?,
        read_u64(handle, 0x270).await?,
    ])
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_wait_returns_shutdown_without_notification() -> TestResult {
    // Given: a pending readiness wait on a device with no notification.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let cancelled = tokio::spawn({
        let handle = handle.clone();
        async move { handle.wait_irq_notification_ready().await }
    });
    tokio::task::yield_now().await;
    cancelled.abort();
    assert!(cancelled.await.is_err_and(|error| error.is_cancelled()));
    let wait = tokio::spawn({
        let handle = handle.clone();
        async move { handle.wait_irq_notification_ready().await }
    });

    // When: device shutdown is requested.
    device.request_shutdown().await;

    // Then: readiness reports shutdown and no notification was synthesized.
    assert!(!wait.await?);
    assert!(!handle.irq_notification_ready().await);
    Ok(())
}
