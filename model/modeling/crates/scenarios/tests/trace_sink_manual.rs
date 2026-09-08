#![allow(missing_docs)]

use pdma_model_contract::semantic::{CompletionStatus, desc::CopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{ManualDriver, compat::AsyncDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn async_tracing_preserves_manual_run_until_idle_behavior() {
    // Given: the same copy submitted through separate baseline and traced observability drivers.
    let (baseline_runner, baseline_worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let mut baseline_port = AsyncDevicePort::new(baseline_runner.handle());
    let mut baseline_driver = ManualDriver::setup_on(&mut baseline_port, 4, 4)
        .await
        .expect("queue setup is valid");
    let baseline_src = baseline_driver
        .allocate_buffer_on(&mut baseline_port, 4)
        .await
        .expect("allocation");
    let baseline_dst = baseline_driver
        .allocate_buffer_on(&mut baseline_port, 4)
        .await
        .expect("allocation");
    baseline_driver
        .write_buffer_on(&mut baseline_port, baseline_src, &[4, 3, 2, 1])
        .await
        .expect("source write is in bounds");
    baseline_driver
        .submit_copy_on(
            &mut baseline_port,
            CopyRequest::new(baseline_src, baseline_dst, 4),
        )
        .await
        .expect("baseline copy submission has ring space");

    let (traced_runner, traced_worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let traced_handle = traced_runner.handle();
    traced_handle.enable_tracing().await;
    traced_handle.clear_trace().await;
    let mut traced_port = AsyncDevicePort::new(traced_handle.clone());
    let mut traced_driver = ManualDriver::setup_on(&mut traced_port, 4, 4)
        .await
        .expect("queue setup is valid");
    let traced_src = traced_driver
        .allocate_buffer_on(&mut traced_port, 4)
        .await
        .expect("allocation");
    let traced_dst = traced_driver
        .allocate_buffer_on(&mut traced_port, 4)
        .await
        .expect("allocation");
    traced_driver
        .write_buffer_on(&mut traced_port, traced_src, &[4, 3, 2, 1])
        .await
        .expect("source write is in bounds");
    traced_driver
        .submit_copy_on(
            &mut traced_port,
            CopyRequest::new(traced_src, traced_dst, 4),
        )
        .await
        .expect("traced copy submission has ring space");

    // When: both actor workers execute and retire their queued copies.
    let baseline_worker = tokio::spawn(baseline_worker.run());
    let traced_worker = tokio::spawn(traced_worker.run());
    let baseline_summary = baseline_runner.wait_for_progress_after(0).await;
    let traced_summary = traced_runner.wait_for_progress_after(0).await;
    let baseline_completion = baseline_driver
        .poll_completion_on(&mut baseline_port)
        .await
        .expect("completion ring exists")
        .expect("baseline completion was published");
    let traced_completion = traced_driver
        .poll_completion_on(&mut traced_port)
        .await
        .expect("completion ring exists")
        .expect("traced completion was published");
    let baseline_bytes = baseline_driver
        .read_buffer_on(&baseline_port, baseline_dst, 4)
        .await;
    let traced_bytes = traced_driver
        .read_buffer_on(&traced_port, traced_dst, 4)
        .await;
    let traced_events = traced_handle.trace_events().await;
    baseline_runner.request_shutdown().await;
    traced_runner.request_shutdown().await;
    baseline_worker
        .await
        .expect("baseline runner joins cleanly");
    traced_worker.await.expect("traced runner joins cleanly");

    // Then: tracing leaves retirement and copied bytes unchanged while recording events.
    assert_eq!(baseline_summary.steps, 1);
    assert_eq!(traced_summary.steps, 1);
    assert_eq!(baseline_completion.status(), CompletionStatus::Success);
    assert_eq!(traced_completion.status(), CompletionStatus::Success);
    assert_eq!(baseline_bytes, Ok(vec![4, 3, 2, 1]));
    assert_eq!(traced_bytes, Ok(vec![4, 3, 2, 1]));
    assert!(!traced_events.is_empty());
}
