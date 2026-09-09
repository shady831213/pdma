#![allow(missing_docs)]

mod support;
mod stall_annotations {
    mod manual;
}

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CompletionStatus, ResourceConfig, ids::CommandId};
use pdma_model_device::{AsyncDevice, AsyncStallReason, AsyncStepStatus};
use pdma_model_host::CopyRequest;
use pdma_model_host::{
    AsyncDriver, DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_no_work_reports_idle_status() {
    // Given: a queue-port async runner with no queued work.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());

    // When: software observes the runner before submitting commands.
    let snapshot = runner.snapshot().await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the public status API reports the idle terminal status.
    assert_eq!(snapshot.last_status, AsyncStepStatus::Idle);
    assert_eq!(summary.last_status, AsyncStepStatus::Idle);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_successful_copy_reports_progressed_status() {
    // Given: an async copy on a queue-port async runner with default resources.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let snapshot = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
                .await
                .expect("source write is in bounds");

            // When: software submits the copy and waits for one hardware progress step.
            driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await
                .expect("copy submission has ring space");
            runner.wait_for_progress_after(0).await
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the progress snapshot reports the progressed status.
    assert_eq!(snapshot.last_status, AsyncStepStatus::Progressed);
    assert_eq!(summary.steps, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_resource_stall_reports_execution_reason() {
    // Given: an async copy on a device with no read slots.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default().with_read_slots(0),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
                .await
                .expect("source write is in bounds");

            // When: software submits the copy and waits for the first stall annotation.
            driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await
                .expect("copy submission has ring space");
            let snapshot = runner.wait_for_stall_after(0).await;
            let completion = driver.poll_completion_on(&mut handle).await;
            (snapshot, completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the public status API names execution resource pressure as the stall reason.
    assert_eq!(
        snapshot.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::ExecutionResourcePressure)
    );
    assert_eq!(summary.stalls, 1);
    assert_eq!(completion, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_completion_publication_stall_reports_reason() {
    // Given: two async copies behind a one-slot completion publication path.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, first_completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
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

            // When: software submits both copies and waits for the publication stall.
            driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
                .await
                .expect("first copy submission has ring space");
            driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
                .await
                .expect("second copy submission has ring space");
            let snapshot = runner.wait_for_stall_after(0).await;
            let first_completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("first completion was published");
            (snapshot, first_completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the public status API names completion publication pressure as the stall reason,
    // and the first completion was already published before the second stalled.
    assert_eq!(
        snapshot.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
    );
    assert_eq!(summary.stalls, 1);
    assert_eq!(first_completion.command_id(), CommandId::new(0));
    assert_eq!(first_completion.status(), CompletionStatus::Success);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_fault_publication_retry_reports_distinct_reasons() {
    // Given: a faulting async copy behind a one-deep event publication path.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default().with_event_publication_slots(0),
    );
    let worker = tokio::spawn(worker.run());
    let (first_snapshot, second_snapshot, event) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let bad_src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let bad_dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");

            // When: software submits the faulting copy and waits across the retry boundary.
            driver
                .submit_copy_on(&mut handle, CopyRequest::new(bad_src, bad_dst, 8))
                .await
                .expect("faulting copy submission has ring space");
            let first_snapshot = runner.wait_for_stall_after(0).await;
            handle
                .ring_doorbell(driver.queue_config_handle())
                .await
                .expect("doorbell rings after first stall");
            let second_snapshot = runner.wait_for_stall_after(first_snapshot.stalls).await;
            let event = driver.poll_event_on(&mut handle).await;
            (first_snapshot, second_snapshot, event)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the first stall is event publication pressure, and the retry reports fault retirement pending retry.
    assert_eq!(
        first_snapshot.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::EventPublicationPressure)
    );
    assert_eq!(
        second_snapshot.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::FaultRetirementPendingRetry)
    );
    assert_eq!(summary.stalls, 2);
    assert_eq!(event, Ok(None));
}
