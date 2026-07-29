#![allow(missing_docs)]

use pdma_model_contract::{
    CommandId, ResourceConfig, TraceEvent, TraceEventKind,
    desc::{CopyCommand, CopyRequest, ExternalCommand},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_does_not_duplicate_success_on_completion_retry() {
    // Given: two traced copies behind a one-slot completion publication path.
    let (runner, worker) = AsyncDevice::queue_port_with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let cpu = SoftwareCpu::new();
    handle.enable_tracing().await;
    handle.clear_trace().await;

    // When: one software main drains the first completion so retry can publish the second.
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
            let first_snapshot = runner.wait_for_progress_after(0).await;
            let _first_completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("first completion was published");
            let _second_snapshot = runner.wait_for_progress_after(first_snapshot.steps).await;
            second_command
        })
        .await;
    let trace_events = handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: retry publishes the completion without duplicating execution success.
    let second_successes = trace_events
        .iter()
        .filter(|event| {
            event.kind() == TraceEventKind::ExecutionSucceeded
                && event.command_id() == Some(second_command)
        })
        .count();
    assert_eq!(second_successes, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_does_not_leak_disabled_submission_metadata() {
    // Given: a command submitted while trace capture is disabled.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let cpu = SoftwareCpu::new();

    // When: instrumentation stays inside main to preserve submit-disabled doorbell ordering.
    let queue_id = cpu
        .run_main(async {
            let driver = AsyncDriver::setup_on(&mut handle, 4, 4)
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
            let command = CopyCommand::new(
                CommandId::new(0),
                driver.queue_id(),
                driver.completion_ring_id(),
                CopyRequest::new(src, dst, 4),
            );
            handle
                .submit_command(driver.queue_config_handle(), ExternalCommand::Copy(command))
                .await
                .expect("direct submission has ring space");
            handle.enable_tracing().await;
            handle.clear_trace().await;
            handle
                .ring_doorbell(driver.queue_config_handle())
                .await
                .expect("doorbell targets an existing queue");
            driver.queue_id()
        })
        .await;
    let trace_events = handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: disabled submission metadata does not leak into later queue-only doorbell traces.
    assert!(
        !trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::CommandSubmitted)
    );
    assert!(trace_events.iter().any(|event| {
        event.kind() == TraceEventKind::DoorbellRunnable
            && event.queue_id() == Some(queue_id)
            && event.operation().is_none()
            && event.command_id().is_none()
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_records_unrelated_queue_event_poll_none() {
    // Given: two traced async drivers where one queue faults and the other remains unrelated.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let cpu = SoftwareCpu::new();
    handle.enable_tracing().await;
    handle.clear_trace().await;

    // When: one software main polls the unrelated queue before the faulting queue.
    let (wrong_driver_event, owner_event, owner_queue_id, unrelated_queue_id) = cpu
        .run_main(async {
            let mut driver_a = AsyncDriver::setup_on(&mut handle, 1, 4)
                .await
                .expect("first queue setup is valid");
            let mut driver_b = AsyncDriver::setup_on(&mut handle, 1, 4)
                .await
                .expect("second queue setup is valid");
            let a_src = driver_a
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let a_dst = driver_a
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            driver_a
                .submit_copy_on(&mut handle, CopyRequest::new(a_src, a_dst, 8))
                .await
                .expect("faulting copy has ring space");
            let _snapshot = runner.wait_for_error_after(0).await;
            let wrong_driver_event = driver_b
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists");
            let owner_event = driver_a
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("owning driver can still poll its event");
            (
                wrong_driver_event,
                owner_event,
                driver_a.queue_id(),
                driver_b.queue_id(),
            )
        })
        .await;
    let trace_events = handle.trace_events().await;
    let trace_json = handle.trace_json().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: the unrelated poll does not consume the event and both poll paths are traced.
    assert_eq!(wrong_driver_event, None);
    assert_eq!(owner_event.queue_id(), owner_queue_id);
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::EventPolled
            && event.queue_id() == Some(unrelated_queue_id)
            && event.detail() == "event poll None"
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::EventPolled
            && event.queue_id() == Some(owner_queue_id)
            && event.detail() == "event poll Some"
    }));
    assert!(trace_json.contains("traceEvents"));
    assert!(trace_json.contains("\"ph\":\"i\""));
    assert!(trace_json.contains("\"ts\""));
    assert!(trace_json.contains("\"pid\""));
    assert!(trace_json.contains("\"tid\""));
}
