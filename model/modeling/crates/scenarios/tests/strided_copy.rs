#![allow(missing_docs)]

#[path = "strided_copy/retry.rs"]
mod retry;

use pdma_model_contract::{CompletionStatus, desc::StridedCopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_retries_strided_completion_without_reexecuting_memory_copy() {
    retry::run_strided_completion_retry_scenario().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copies_rows_end_to_end_when_strides_include_padding() {
    // Given: async runner handle, AsyncDriver, padded source rows, and sentinel destination bytes.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let (snapshot, completion, command_id, queue_id, dst_bytes, no_completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = runner.handle();
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 16)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 20)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(
                    &mut handle,
                    src,
                    &[1, 2, 3, 9, 4, 5, 6, 9, 7, 8, 9, 9, 0, 0, 0, 0],
                )
                .await
                .expect("source write is in bounds");
            driver
                .write_buffer_on(&mut handle, dst, &[0xEE; 20])
                .await
                .expect("destination write is in bounds");

            // When: the software main submits the strided copy, waits, and polls completion.
            let command_id = driver
                .submit_strided_copy_on(&mut handle, StridedCopyRequest::new(src, dst, 3, 3, 4, 6))
                .await
                .expect("strided copy submission has ring space");
            let snapshot = runner.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("completion was published");
            let queue_id = driver.queue_id();
            let dst_bytes = driver.read_buffer_on(&handle, dst, 20).await;
            let no_completion = driver.poll_completion_on(&mut handle).await;
            (
                snapshot,
                completion,
                command_id,
                queue_id,
                dst_bytes,
                no_completion,
            )
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: row payloads are copied, padding stays unchanged, and one completion appears.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(completion.queue_id(), queue_id);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        dst_bytes,
        Ok(vec![
            1, 2, 3, 0xEE, 0xEE, 0xEE, 4, 5, 6, 0xEE, 0xEE, 0xEE, 7, 8, 9, 0xEE, 0xEE, 0xEE, 0xEE,
            0xEE,
        ])
    );
    assert_eq!(no_completion, Ok(None));
    assert_eq!(summary.steps, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_reports_out_of_bounds_when_strided_row_exceeds_source_buffer() {
    // Given: async runner handle, AsyncDriver, and a strided copy whose second row is too long.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let (snapshot, no_completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = runner.handle();
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let request = StridedCopyRequest::new(src, dst, 2, 4, 6, 4);
            driver
                .submit_strided_copy_on(&mut handle, request)
                .await
                .expect("strided copy submission has ring space");

            // When: the async runner executes the command.
            let snapshot = runner.wait_for_error_after(0).await;
            let no_completion = driver.poll_completion_on(&mut handle).await;
            (snapshot, no_completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: checked memory reports the failing row and no completion is published.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(no_completion, Ok(None));
    assert_eq!(summary.errors, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_keeps_queue_runnable_when_failed_strided_copy_has_later_work() {
    // Given: a bad strided copy followed by a valid strided copy in the same async queue.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let (error_snapshot, progress_snapshot, completion, good_command, good_dst_bytes) =
        SoftwareCpu::new()
            .run_main(async {
                let mut handle = runner.handle();
                let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                    .await
                    .expect("queue setup is valid");
                let bad_src = driver
                    .allocate_buffer_on(&mut handle, 8)
                    .await
                    .expect("allocation");
                let good_src = driver
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                let bad_dst = driver
                    .allocate_buffer_on(&mut handle, 8)
                    .await
                    .expect("allocation");
                let good_dst = driver
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                driver
                    .write_buffer_on(&mut handle, good_src, &[0xA0, 0xA1, 0xA2, 0xA3])
                    .await
                    .expect("source write is in bounds");
                driver
                    .submit_strided_copy_on(
                        &mut handle,
                        StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
                    )
                    .await
                    .expect("bad command has ring space");
                let good_command = driver
                    .submit_strided_copy_on(
                        &mut handle,
                        StridedCopyRequest::new(good_src, good_dst, 1, 4, 4, 4),
                    )
                    .await
                    .expect("good command has ring space");

                // When: the first command fails and the async runner later completes the valid command.
                let error_snapshot = runner.wait_for_error_after(0).await;
                let progress_snapshot = runner.wait_for_progress_after(0).await;
                let completion = driver
                    .poll_completion_on(&mut handle)
                    .await
                    .expect("completion ring exists")
                    .expect("good command completed");
                let good_dst_bytes = driver.read_buffer_on(&handle, good_dst, 4).await;
                (
                    error_snapshot,
                    progress_snapshot,
                    completion,
                    good_command,
                    good_dst_bytes,
                )
            })
            .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the bad command reports its error, while the later command still completes.
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(progress_snapshot.steps, 1);
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(good_dst_bytes, Ok(vec![0xA0, 0xA1, 0xA2, 0xA3]));
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.steps, 1);
}
