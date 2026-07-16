#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, desc::CopyRequest};
use pdma_model_device::SyncDevice;
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_tracing_preserves_manual_run_until_idle_behavior() {
    // Given: the same copy on a baseline manual device and a traced manual device.
    let mut baseline_device = SyncDevice::new();
    let mut baseline_driver = AsyncDriver::setup_manual_on(&mut baseline_device, 4, 4)
        .await
        .expect("queue setup is valid");
    let baseline_src = baseline_driver
        .allocate_buffer_on(&mut baseline_device, 4)
        .await;
    let baseline_dst = baseline_driver
        .allocate_buffer_on(&mut baseline_device, 4)
        .await;
    baseline_driver
        .write_buffer_on(&mut baseline_device, baseline_src, &[4, 3, 2, 1])
        .await
        .expect("source write is in bounds");

    let mut traced_device = SyncDevice::new();
    let mut traced_driver = AsyncDriver::setup_manual_on(&mut traced_device, 4, 4)
        .await
        .expect("queue setup is valid");
    let traced_src = traced_driver
        .allocate_buffer_on(&mut traced_device, 4)
        .await;
    let traced_dst = traced_driver
        .allocate_buffer_on(&mut traced_device, 4)
        .await;
    traced_driver
        .write_buffer_on(&mut traced_device, traced_src, &[4, 3, 2, 1])
        .await
        .expect("source write is in bounds");
    traced_device.enable_tracing();
    traced_device.clear_trace();

    // When: both devices run the same scenario to idle through the manual async surface.
    baseline_driver
        .submit_copy_on(
            &mut baseline_device,
            CopyRequest::new(baseline_src, baseline_dst, 4),
        )
        .await
        .expect("baseline copy submission has ring space");
    traced_driver
        .submit_copy_on(
            &mut traced_device,
            CopyRequest::new(traced_src, traced_dst, 4),
        )
        .await
        .expect("traced copy submission has ring space");
    let baseline_summary = baseline_device
        .run_until_idle(1)
        .expect("baseline device runs to idle");
    let traced_summary = traced_device
        .run_until_idle(1)
        .expect("traced device still runs to idle");
    let baseline_completion = baseline_driver
        .poll_completion_on(&mut baseline_device)
        .await
        .expect("completion ring exists")
        .expect("baseline copy completion was published");
    let traced_completion = traced_driver
        .poll_completion_on(&mut traced_device)
        .await
        .expect("completion ring exists")
        .expect("traced copy completion was published");

    // Then: tracing does not change run-until-idle behavior or the completed data path.
    assert_eq!(baseline_summary.steps, 1);
    assert_eq!(traced_summary.steps, 1);
    assert_eq!(baseline_completion.status(), CompletionStatus::Success);
    assert_eq!(traced_completion.status(), CompletionStatus::Success);
    assert_eq!(
        baseline_driver
            .read_buffer_on(&baseline_device, baseline_dst, 4)
            .await,
        Ok(vec![4, 3, 2, 1])
    );
    assert_eq!(
        traced_driver
            .read_buffer_on(&traced_device, traced_dst, 4)
            .await,
        Ok(vec![4, 3, 2, 1])
    );
    assert!(!traced_device.trace_events().is_empty());
}
