#![allow(missing_docs)]

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CompletionStatus, ResourceConfig, desc::CopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, compat::AsyncDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_completion_publication_capacity_one_preserves_completion_order_after_polling()
{
    // Given: two async copies queued behind a one-slot completion publication path.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_completion_publication_slots(1),
    ));
    let worker = tokio::spawn(worker.run());
    let (
        first_command,
        second_command,
        first_snapshot,
        first_completion,
        second_snapshot,
        second_completion,
        final_completion,
    ) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = AsyncDevicePort::new(runner.handle());
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src_a = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst_a = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let src_b = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst_b = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, src_a, &[10, 11, 12, 13])
                .await
                .expect("source A write is in bounds");
            driver
                .write_buffer_on(&mut handle, src_b, &[20, 21, 22, 23])
                .await
                .expect("source B write is in bounds");

            // When: software submits both copies, fills completion publication, then polls before retrying.
            let first_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
                .await
                .expect("first copy submission has ring space");
            let second_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
                .await
                .expect("second copy submission has ring space");
            let first_snapshot = runner.wait_for_progress_after(0).await;
            let first_completion = driver
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
            let final_completion = driver.poll_completion_on(&mut handle).await;
            (
                first_command,
                second_command,
                first_snapshot,
                first_completion,
                second_snapshot,
                second_completion,
                final_completion,
            )
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: retirement stays ordered across polling.
    assert_eq!(first_snapshot.steps, 1);
    assert_eq!(first_completion.command_id(), first_command);
    assert_eq!(first_completion.status(), CompletionStatus::Success);
    assert!(second_snapshot.stalls >= 1);
    assert_eq!(second_snapshot.steps, 2);
    assert_eq!(summary.steps, 2);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_completion.status(), CompletionStatus::Success);
    assert_eq!(final_completion, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_event_publication_capacity_one_preserves_event_order_after_polling() {
    // Given: two faulting async copies behind a one-slot event publication path.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_event_publication_slots(1),
    ));
    let worker = tokio::spawn(worker.run());
    let (
        first_command,
        second_command,
        first_snapshot,
        first_event,
        second_snapshot,
        second_event,
        final_event,
    ) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = AsyncDevicePort::new(runner.handle());
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let bad_src_a = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let bad_dst_a = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let bad_src_b = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let bad_dst_b = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");

            // When: software submits both invalid copies, fills event publication, then polls before retrying.
            let first_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(bad_src_a, bad_dst_a, 8))
                .await
                .expect("first faulting copy has ring space");
            let second_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(bad_src_b, bad_dst_b, 8))
                .await
                .expect("second faulting copy has ring space");
            let first_snapshot = runner.wait_for_error_after(0).await;
            let first_event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("first error event was published");
            let second_snapshot = runner.wait_for_error_after(first_snapshot.errors).await;
            let second_event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("second error event was published");
            let final_event = driver.poll_event_on(&mut handle).await;
            (
                first_command,
                second_command,
                first_snapshot,
                first_event,
                second_snapshot,
                second_event,
                final_event,
            )
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: error publication stays ordered across polling.
    assert_eq!(first_snapshot.errors, 1);
    assert_eq!(first_event.command_id(), first_command);
    assert!(second_snapshot.stalls >= 1);
    assert_eq!(second_snapshot.errors, 2);
    assert_eq!(summary.errors, 2);
    assert_eq!(second_event.command_id(), second_command);
    assert_eq!(final_event, Ok(None));
}
