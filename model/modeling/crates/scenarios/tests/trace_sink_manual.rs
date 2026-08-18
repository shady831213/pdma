#![allow(missing_docs)]

use pdma_model_contract::semantic::{CompletionStatus, desc::CopyRequest};
use pdma_model_device::SyncDevice;
use pdma_model_host::{ManualDriver, compat::SemanticDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn async_tracing_preserves_manual_run_until_idle_behavior() {
    // Given: the same copy on a baseline manual device and a traced manual device.
    let mut baseline_device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let (mut baseline_driver, baseline_dst) = {
        let mut port = SemanticDevicePort::new(&mut baseline_device);
        let mut driver = ManualDriver::setup_on(&mut port, 4, 4)
            .await
            .expect("queue setup is valid");
        let src = driver
            .allocate_buffer_on(&mut port, 4)
            .await
            .expect("allocation");
        let dst = driver
            .allocate_buffer_on(&mut port, 4)
            .await
            .expect("allocation");
        driver
            .write_buffer_on(&mut port, src, &[4, 3, 2, 1])
            .await
            .expect("source write is in bounds");
        driver
            .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
            .await
            .expect("baseline copy submission has ring space");
        (driver, dst)
    };

    let mut traced_device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    traced_device.enable_tracing();
    traced_device.clear_trace();
    let (mut traced_driver, traced_dst) = {
        let mut port = SemanticDevicePort::new(&mut traced_device);
        let mut driver = ManualDriver::setup_on(&mut port, 4, 4)
            .await
            .expect("queue setup is valid");
        let src = driver
            .allocate_buffer_on(&mut port, 4)
            .await
            .expect("allocation");
        let dst = driver
            .allocate_buffer_on(&mut port, 4)
            .await
            .expect("allocation");
        driver
            .write_buffer_on(&mut port, src, &[4, 3, 2, 1])
            .await
            .expect("source write is in bounds");
        driver
            .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
            .await
            .expect("traced copy submission has ring space");
        (driver, dst)
    };

    // When: both devices run the same scenario to idle through the manual async surface.
    let baseline_summary = baseline_device
        .run_until_idle(1)
        .expect("baseline device runs to idle");
    let traced_summary = traced_device
        .run_until_idle(1)
        .expect("traced device still runs to idle");
    let (baseline_completion, baseline_bytes) = {
        let mut port = SemanticDevicePort::new(&mut baseline_device);
        let completion = baseline_driver
            .poll_completion_on(&mut port)
            .await
            .expect("completion ring exists")
            .expect("baseline copy completion was published");
        let bytes = baseline_driver.read_buffer_on(&port, baseline_dst, 4).await;
        (completion, bytes)
    };
    let (traced_completion, traced_bytes) = {
        let mut port = SemanticDevicePort::new(&mut traced_device);
        let completion = traced_driver
            .poll_completion_on(&mut port)
            .await
            .expect("completion ring exists")
            .expect("traced copy completion was published");
        let bytes = traced_driver.read_buffer_on(&port, traced_dst, 4).await;
        (completion, bytes)
    };

    // Then: tracing does not change run-until-idle behavior or the completed data path.
    assert_eq!(baseline_summary.steps, 1);
    assert_eq!(traced_summary.steps, 1);
    assert_eq!(baseline_completion.status(), CompletionStatus::Success);
    assert_eq!(traced_completion.status(), CompletionStatus::Success);
    assert_eq!(baseline_bytes, Ok(vec![4, 3, 2, 1]));
    assert_eq!(traced_bytes, Ok(vec![4, 3, 2, 1]));
    assert!(!traced_device.trace_events().is_empty());
}
