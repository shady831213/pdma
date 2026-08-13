use pdma_model_contract::{
    CompletionStatus, EventErrorKind,
    desc::{CopyRequest, StridedCopyRequest},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_many_memory_fault_events_without_masking_original_error() {
    // Given: more invalid async commands than the previous hidden event-ring depth.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let (snapshot, error_kinds, no_event) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = runner.handle();
            let mut driver = AsyncDriver::setup_on(&mut handle, 65, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            for _ in 0..65 {
                driver
                    .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
                    .await
                    .expect("command ring has enough space");
            }

            // When: every invalid command is executed without polling events between failures.
            let snapshot = runner.wait_for_error_after(64).await;
            let mut error_kinds = Vec::with_capacity(65);
            for _ in 0..65 {
                let event = driver
                    .poll_event_on(&mut handle)
                    .await
                    .expect("event polling succeeds")
                    .expect("one event exists for each failed command");
                error_kinds.push(event.error_kind());
            }
            let no_event = driver.poll_event_on(&mut handle).await;
            (snapshot, error_kinds, no_event)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: every fault keeps its original error and produces a pollable event.
    assert_eq!(snapshot.errors, 65);
    for error_kind in error_kinds {
        assert_eq!(error_kind, EventErrorKind::OutOfBounds { len: 4, end: 8 });
    }
    assert_eq!(no_event, Ok(None));
    assert_eq!(summary.errors, 65);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_one_event_and_later_command_still_completes_after_bad_strided_copy()
{
    // Given: async runner handle, AsyncDriver, and a bad strided copy before a valid linear copy.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let (
        error_snapshot,
        progress_snapshot,
        event,
        bad_command,
        bad_src,
        completion,
        good_command,
        good_dst_bytes,
        no_event,
    ) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = runner.handle();
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let bad_src = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let bad_dst = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let good_src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let good_dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, good_src, &[0x10, 0x11, 0x12, 0x13])
                .await
                .expect("source write is in bounds");

            // When: the first command fails and the async runner later completes the valid command.
            let bad_command = driver
                .submit_strided_copy_on(
                    &mut handle,
                    StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
                )
                .await
                .expect("bad command has ring space");
            let good_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 4))
                .await
                .expect("good command has ring space");
            let error_snapshot = runner.wait_for_error_after(0).await;
            let event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("bad command event was published");
            let progress_snapshot = runner.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("good command completed");
            let good_dst_bytes = driver.read_buffer_on(&handle, good_dst, 4).await;
            let no_event = driver.poll_event_on(&mut handle).await;
            (
                error_snapshot,
                progress_snapshot,
                event,
                bad_command,
                bad_src,
                completion,
                good_command,
                good_dst_bytes,
                no_event,
            )
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the failed command publishes one event, while the later command completes.
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(progress_snapshot.steps, 1);
    assert_eq!(event.command_id(), bad_command);
    assert_eq!(event.buffer(), bad_src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 8, end: 10 }
    );
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(good_dst_bytes, Ok(vec![0x10, 0x11, 0x12, 0x13]));
    assert_eq!(no_event, Ok(None));
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.steps, 1);
}
