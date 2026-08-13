use super::*;

#[allow(clippy::expect_used, clippy::redundant_pub_crate)]
pub(super) async fn run_strided_completion_retry_scenario() {
    // Given: two strided copies sharing a one-slot completion ring.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let (stalled_snapshot, second_snapshot, second_completion, second_command, second_dst_bytes) =
        SoftwareCpu::new()
            .run_main(async {
                let mut handle = runner.handle();
                let mut driver = AsyncDriver::setup_on(&mut handle, 4, 1)
                    .await
                    .expect("queue setup is valid");
                let first_src = driver
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                let first_dst = driver
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                let second_src = driver
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                let second_dst = driver
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                driver
                    .write_buffer_on(&mut handle, first_src, &[1, 2, 3, 4])
                    .await
                    .expect("first source write is in bounds");
                driver
                    .write_buffer_on(&mut handle, second_src, &[5, 6, 7, 8])
                    .await
                    .expect("second source write is in bounds");
                driver
                    .submit_strided_copy_on(
                        &mut handle,
                        StridedCopyRequest::new(first_src, first_dst, 1, 4, 4, 4),
                    )
                    .await
                    .expect("first command has ring space");
                let second_command = driver
                    .submit_strided_copy_on(
                        &mut handle,
                        StridedCopyRequest::new(second_src, second_dst, 1, 4, 4, 4),
                    )
                    .await
                    .expect("second command has ring space");

                // When: the second copy stalls on completion publication, then software mutates its source.
                let first_snapshot = runner.wait_for_progress_after(0).await;
                let stalled_snapshot = if first_snapshot.stalls == 0 {
                    runner.wait_for_change_after(first_snapshot).await
                } else {
                    first_snapshot
                };
                driver
                    .write_buffer_on(&mut handle, second_src, &[9, 9, 9, 9])
                    .await
                    .expect("second source rewrite is in bounds");
                let _first_completion = driver
                    .poll_completion_on(&mut handle)
                    .await
                    .expect("completion ring exists")
                    .expect("first completion was published");
                let second_snapshot = runner.wait_for_progress_after(first_snapshot.steps).await;
                let second_completion = driver
                    .poll_completion_on(&mut handle)
                    .await
                    .expect("completion ring exists")
                    .expect("second completion was published");
                let second_dst_bytes = driver.read_buffer_on(&handle, second_dst, 4).await;
                (
                    stalled_snapshot,
                    second_snapshot,
                    second_completion,
                    second_command,
                    second_dst_bytes,
                )
            })
            .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: completion retry publishes the executed copy without copying mutated bytes.
    assert!(stalled_snapshot.stalls >= 1);
    assert_eq!(second_snapshot.steps, 2);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_dst_bytes, Ok(vec![5, 6, 7, 8]));
    assert_eq!(summary.steps, 2);
}
