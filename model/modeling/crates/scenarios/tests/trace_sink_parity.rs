#![allow(missing_docs)]

use pdma_model_contract::{
    ResourceConfig, TraceEvent, TraceEventKind,
    desc::{CopyRequest, VectorAddRequest, VectorBufferRange},
};
use pdma_model_device::{AsyncDevice, AsyncStepStatus};
use pdma_model_host::{AsyncDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_records_idle_status_steps() {
    // Given: an idle async runner with tracing enabled after startup noise is cleared.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    handle.enable_tracing().await;
    handle.clear_trace().await;
    let baseline = handle.snapshot().await;

    // When: the worker observes another idle turn after the trace wake.
    let snapshot = runner.wait_for_change_after(baseline).await;
    let trace_events = handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: the trace mirrors sync idle status observability.
    assert_eq!(snapshot.last_status, AsyncStepStatus::Idle);
    assert!(trace_events.iter().any(|event| {
        event.kind() == TraceEventKind::Idle
            && event.operation().is_none()
            && event.command_id().is_none()
            && event.detail() == "idle"
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_schedules_before_resource_stall() {
    // Given: a traced async runner whose resources cannot satisfy a copy read.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_read_slots(0),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let cpu = SoftwareCpu::new();
    handle.enable_tracing().await;
    handle.clear_trace().await;

    // When: one software main submits the copy and waits for its first stall.
    let command = cpu
        .run_main(async {
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
            let command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await
                .expect("copy submission has ring space");
            let _snapshot = runner.wait_for_stall_after(0).await;
            command
        })
        .await;
    let trace_events = handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: scheduling is recorded before resource pressure stalls the command.
    let scheduled = event_position(&trace_events, TraceEventKind::Scheduled, command)
        .expect("scheduled event is traced");
    let stall = trace_events
        .iter()
        .position(|event| {
            event.kind() == TraceEventKind::Stall && event.detail() == "ExecutionResourcePressure"
        })
        .expect("resource stall is traced");
    assert!(scheduled < stall);
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_preserves_success_before_completion_pressure_stall() {
    // Given: two traced copies behind a one-slot completion publication path.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_completion_publication_slots(1),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let cpu = SoftwareCpu::new();
    handle.enable_tracing().await;
    handle.clear_trace().await;

    // When: one software main fills completion capacity and waits for the second stall.
    let second_command = cpu
        .run_main(async {
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
                .write_buffer_on(&mut handle, src_a, &[1, 2, 3, 4])
                .await
                .expect("source A write is in bounds");
            driver
                .write_buffer_on(&mut handle, src_b, &[5, 6, 7, 8])
                .await
                .expect("source B write is in bounds");
            let _first_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
                .await
                .expect("first copy submission has ring space");
            let second_command = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
                .await
                .expect("second copy submission has ring space");
            let _first_snapshot = runner.wait_for_progress_after(0).await;
            let _second_snapshot = runner.wait_for_stall_after(0).await;
            second_command
        })
        .await;
    let trace_events = handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: execution success for the second command is retained before publication pressure.
    let succeeded = event_position(
        &trace_events,
        TraceEventKind::ExecutionSucceeded,
        second_command,
    )
    .expect("second execution success is traced");
    let stall = trace_events
        .iter()
        .position(|event| {
            event.kind() == TraceEventKind::Stall
                && event.detail() == "CompletionPublicationPressure"
        })
        .expect("completion publication stall is traced");
    assert!(succeeded < stall);
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_records_queue_only_doorbells() {
    // Given: two drivers whose first commands reuse the same command id on distinct queues.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let cpu = SoftwareCpu::new();
    handle.enable_tracing().await;
    handle.clear_trace().await;

    // When: one software main submits command id zero on both queues.
    let (copy_command, vector_command, vector_queue_id) = cpu
        .run_main(async {
            let mut copy_driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("copy queue setup is valid");
            let mut vector_driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("vector queue setup is valid");
            let copy_src = copy_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let copy_dst = copy_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let src_a = vector_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let src_b = vector_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let vector_dst = vector_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let copy_command = copy_driver
                .submit_copy_on(&mut handle, CopyRequest::new(copy_src, copy_dst, 4))
                .await
                .expect("copy submission has ring space");
            let vector_command = vector_driver
                .submit_vector_add_on(
                    &mut handle,
                    VectorAddRequest::new(
                        VectorBufferRange::new(src_a, 0),
                        VectorBufferRange::new(src_b, 0),
                        VectorBufferRange::new(vector_dst, 0),
                        4,
                    ),
                )
                .await
                .expect("vector submission has ring space");
            (copy_command, vector_command, vector_driver.queue_id())
        })
        .await;
    let trace_events = handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: doorbells stay queue-only and cannot be mislabeled by reused command ids.
    assert_eq!(copy_command, vector_command);
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::DoorbellRunnable
            && event.queue_id() == Some(vector_queue_id)
            && event.command_id().is_none()
            && event.operation().is_none()
    }));
}

fn event_position(
    events: &[TraceEvent],
    kind: TraceEventKind,
    command: pdma_model_contract::ids::CommandId,
) -> Option<usize> {
    events
        .iter()
        .position(|event| event.kind() == kind && event.command_id() == Some(command))
}
