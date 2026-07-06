#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, ResourceConfig,
    desc::{
        BroadcastCopyRequest, BroadcastDestinationRange, CopyRequest, VectorAddRequest,
        VectorBufferRange,
    },
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copy_stalls_without_read_slots() {
    // Given: an async copy on a device with no read slots.
    let runner = AsyncDevice::spawn_native_with_resource_config(
        ResourceConfig::default().with_read_slots(0),
    );
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");
    let mut snapshot = runner.snapshot().await;

    // When: software submits the copy and the async runner tries to retire it.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    while snapshot.stalls == 0 {
        snapshot = runner.wait_for_change_after(snapshot).await;
    }
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the command stays pending because the read slot budget is empty.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copy_completes_with_one_read_slot() {
    // Given: an async copy on a device with one read slot available.
    let runner = AsyncDevice::spawn_native_with_resource_config(
        ResourceConfig::default().with_read_slots(1),
    );
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");

    // When: software submits the copy and the async runner retires it.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("copy published one completion");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the copy completes successfully and the destination matches the source.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 4).await,
        Ok(vec![1, 2, 3, 4])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_broadcast_stalls_with_insufficient_write_and_descriptor_credit() {
    // Given: an async broadcast on a device with insufficient write and descriptor credit.
    let runner = AsyncDevice::spawn_native_with_resource_config(
        ResourceConfig::default()
            .with_write_slots(1)
            .with_internal_descriptors(1),
    );
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst_a = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst_b = driver.allocate_buffer_on(&mut handle, 8).await;
    driver
        .write_buffer_on(&mut handle, src, &[0, 1, 2, 3, 4, 5, 6, 7])
        .await
        .expect("source write is in bounds");
    let mut snapshot = runner.snapshot().await;

    // When: software submits one broadcast command and the async runner tries to retire it.
    let command_id = driver
        .submit_broadcast_copy_on(
            &mut handle,
            BroadcastCopyRequest::new(
                src,
                2,
                4,
                vec![
                    BroadcastDestinationRange::new(dst_a, 0),
                    BroadcastDestinationRange::new(dst_b, 0),
                ],
            ),
        )
        .await
        .expect("broadcast submission has ring space");
    while snapshot.stalls == 0 {
        snapshot = runner.wait_for_change_after(snapshot).await;
    }
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the command stays pending instead of retiring with too little credit.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_vector_add_stalls_without_compute_slots() {
    // Given: an async vector-add command on a device with no compute slots.
    let runner = AsyncDevice::spawn_native_with_resource_config(
        ResourceConfig::default().with_compute_slots(0),
    );
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 8).await;
    let src_b = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst = driver.allocate_buffer_on(&mut handle, 8).await;
    let mut snapshot = runner.snapshot().await;

    // When: software submits the vector add and the async runner tries to retire it.
    let command_id = driver
        .submit_vector_add_on(
            &mut handle,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 0),
                VectorBufferRange::new(src_b, 0),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .await
        .expect("vector-add submission has ring space");
    while snapshot.stalls == 0 {
        snapshot = runner.wait_for_change_after(snapshot).await;
    }
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the command stays pending because no compute slot is available.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_vector_add_stalls_with_one_read_slot() {
    // Given: an async vector-add command on a device with only one read slot.
    let runner = AsyncDevice::spawn_native_with_resource_config(
        ResourceConfig::default().with_read_slots(1),
    );
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 8).await;
    let src_b = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst = driver.allocate_buffer_on(&mut handle, 8).await;
    let mut snapshot = runner.snapshot().await;

    // When: software submits the vector add and the async runner tries to retire it.
    let command_id = driver
        .submit_vector_add_on(
            &mut handle,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 0),
                VectorBufferRange::new(src_b, 0),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .await
        .expect("vector-add submission has ring space");
    while snapshot.stalls == 0 {
        snapshot = runner.wait_for_change_after(snapshot).await;
    }
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the command stays pending because vector-add needs two read slots.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}
