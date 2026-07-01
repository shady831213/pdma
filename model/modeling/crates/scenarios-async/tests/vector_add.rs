#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, EventErrorKind,
    desc::{VectorAddRequest, VectorBufferRange},
};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_adds_bytes_with_wraparound_and_publishes_one_completion() {
    // Given: async runner handle, AsyncDriver, two source buffers, and sentinel destination bytes.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 12).await;
    let src_b = driver.allocate_buffer_on(&mut handle, 12).await;
    let dst = driver.allocate_buffer_on(&mut handle, 16).await;
    driver
        .write_buffer_on(
            &mut handle,
            src_a,
            &[
                0xA0, 0xA1, 1, 250, 0, 127, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
            ],
        )
        .await
        .expect("source A write is in bounds");
    driver
        .write_buffer_on(
            &mut handle,
            src_b,
            &[
                0xB0, 0xB1, 2, 10, 255, 1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7,
            ],
        )
        .await
        .expect("source B write is in bounds");
    driver
        .write_buffer_on(&mut handle, dst, &[0xCC; 16])
        .await
        .expect("destination write is in bounds");

    // When: submit vector-add via AsyncDriver, wait for progress, poll completion, shutdown/join.
    let command_id = driver
        .submit_vector_add_on(
            &mut handle,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 2),
                VectorBufferRange::new(src_b, 2),
                VectorBufferRange::new(dst, 6),
                4,
            ),
        )
        .await
        .expect("vector-add submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("vector-add published one completion");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: byte-wise wrapping addition lands at the destination offset and only one success completion appears.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 16).await,
        Ok(vec![
            0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 3, 4, 255, 128, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_event_and_no_completion_when_vector_add_source_b_read_fails() {
    // Given: async runner handle, AsyncDriver, and a vector add whose source B read is too long.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 8).await;
    let src_b = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 8).await;

    // When: the invalid vector add is submitted and the async runner executes it.
    let command_id = driver
        .submit_vector_add_on(
            &mut handle,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 1),
                VectorBufferRange::new(src_b, 1),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .await
        .expect("vector-add submission has ring space");
    let snapshot = runner.wait_for_error_after(0).await;
    let event = driver
        .poll_event_on(&mut handle)
        .await
        .expect("event ring exists")
        .expect("source-B fault event was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the original out-of-bounds event is published and no success completion appears.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src_b);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 5 }
    );
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}
