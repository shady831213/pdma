#![allow(missing_docs)]

use pdma_model_contract::{
    EventRouting, ModelError, QueueConfig, QueueId, SoftwareOwnerId,
    desc::{CopyRequest, StridedCopyRequest},
};
use pdma_model_device::AsyncDeviceRunner;
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_driver_creates_distinct_queue_handles_from_explicit_configs() {
    // Given: two explicit async queue configurations with different software owners.
    let runner = AsyncDeviceRunner::spawn_native();
    let mut handle = runner.handle();
    let config_a = QueueConfig::new(2, 3, EventRouting::QueueLocal, SoftwareOwnerId::new(10));
    let config_b = QueueConfig::new(4, 5, EventRouting::QueueLocal, SoftwareOwnerId::new(20));

    // When: async drivers create both queues through the port-backed setup API.
    let driver_a = AsyncDriver::setup_with_config_on(&mut handle, config_a)
        .await
        .expect("first queue config is valid");
    let driver_b = AsyncDriver::setup_with_config_on(&mut handle, config_b)
        .await
        .expect("second queue config is valid");
    runner.request_shutdown().await;
    runner.join().await.expect("runner joins cleanly");

    // Then: IDs and completion bindings come from stable queue handles.
    let handle_a = driver_a.queue_config_handle();
    let handle_b = driver_b.queue_config_handle();
    assert_eq!(handle_a.queue_id(), QueueId::new(0));
    assert_eq!(handle_b.queue_id(), QueueId::new(1));
    assert_ne!(handle_a.completion_ring_id(), handle_b.completion_ring_id());
    assert_eq!(
        handle_a.directory_entry(),
        driver_a.queue_config_handle().directory_entry()
    );
    assert_eq!(
        handle_b.directory_entry(),
        driver_b.queue_config_handle().directory_entry()
    );
    assert_eq!(handle_a.config(), config_a);
    assert_eq!(handle_b.config(), config_b);
}

#[tokio::test(flavor = "current_thread")]
async fn async_driver_enforces_command_depth_from_explicit_queue_config() {
    // Given: an async driver created from an explicit one-slot queue config.
    let runner = AsyncDeviceRunner::spawn_native();
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_with_config_on(
        &mut handle,
        QueueConfig::new(1, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
    )
    .await
    .expect("queue config is valid");
    let src = driver.allocate_buffer_on(&mut handle, 1).await;
    let dst = driver.allocate_buffer_on(&mut handle, 1).await;

    // When: software submits one command and tries to submit another before reclaim.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
        .await
        .expect("first command has ring space");
    let second_submit = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
        .await;
    runner.request_shutdown().await;
    runner.join().await.expect("runner joins cleanly");

    // Then: the explicit command depth is enforced and the handle records ownership.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(second_submit, Err(ModelError::DriverQueueFull));
    assert_eq!(driver.queue_config_handle().command_depth(), 1);
    assert_eq!(driver.queue_config_handle().completion_depth(), 2);
    assert_eq!(
        driver.queue_config_handle().software_owner(),
        SoftwareOwnerId::new(7)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_driver_uses_config_handle_for_queue_local_event_polling() {
    // Given: two async drivers with explicit queue-local event routing.
    let runner = AsyncDeviceRunner::spawn_native();
    let mut handle = runner.handle();
    let mut driver_a = AsyncDriver::setup_with_config_on(
        &mut handle,
        QueueConfig::new(1, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(1)),
    )
    .await
    .expect("first queue config is valid");
    let mut driver_b = AsyncDriver::setup_with_config_on(
        &mut handle,
        QueueConfig::new(1, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(2)),
    )
    .await
    .expect("second queue config is valid");
    let bad_src = driver_a.allocate_buffer_on(&mut handle, 4).await;
    let bad_dst = driver_a.allocate_buffer_on(&mut handle, 4).await;
    let good_src = driver_b.allocate_buffer_on(&mut handle, 1).await;
    let good_dst = driver_b.allocate_buffer_on(&mut handle, 1).await;
    driver_a
        .submit_strided_copy_on(
            &mut handle,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .await
        .expect("bad command has ring space");
    let snapshot = runner.wait_for_error_after(0).await;

    // When: the non-owning async driver polls events before the owning driver.
    let wrong_driver_event = driver_b
        .poll_event_on(&mut handle)
        .await
        .expect("event polling succeeds");
    let owner_event = driver_a
        .poll_event_on(&mut handle)
        .await
        .expect("event polling succeeds")
        .expect("owning queue event remains available");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: queue-local routing preserves ownership and only the owning slot is reclaimed.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(wrong_driver_event, None);
    assert_eq!(owner_event.queue_id(), driver_a.queue_id());
    assert!(
        driver_a
            .submit_copy_on(&mut handle, CopyRequest::new(bad_src, bad_dst, 1))
            .await
            .is_ok()
    );
    assert!(
        driver_b
            .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 1))
            .await
            .is_ok()
    );
    assert_eq!(
        driver_b
            .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 1))
            .await,
        Err(ModelError::DriverQueueFull)
    );
}
