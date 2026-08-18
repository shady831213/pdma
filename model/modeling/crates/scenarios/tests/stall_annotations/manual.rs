use pdma_model_device::{AsyncRunSummary, ModelError, SyncDevice};
use pdma_model_host::{ManualDriver, compat::SemanticDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors() {
    // Given: a manual async device with one copy that cannot acquire execution resources.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_read_slots(0),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    {
        let mut port = SemanticDevicePort::new(&mut device);
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
            .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
            .await
            .expect("copy submission has ring space");
    }

    // When: software runs until blocked and then checks the legacy idle API.
    let blocked = device.run_until_blocked(1);
    let idle = device.run_until_idle(1);

    // Then: the blocked API names the stall while the idle API still reports budget exhaustion.
    assert_eq!(
        blocked,
        Ok(AsyncRunSummary {
            steps: 0,
            errors: 0,
            stalls: 1,
            last_status: AsyncStepStatus::Stalled(AsyncStallReason::ExecutionResourcePressure),
        })
    );
    assert_eq!(idle, Err(ModelError::StepBudgetExhausted { max_steps: 1 }));
}
