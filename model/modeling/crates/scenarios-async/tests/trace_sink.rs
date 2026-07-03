#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, ResourceConfig, TraceEvent, TraceEventKind, TraceOperation, desc::CopyRequest,
};
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_records_complete_copy_lifecycle() {
    // Given: an async runner, a driver handle, source/destination buffers, and tracing enabled.
    let runner = AsyncDeviceRunner::spawn_native();
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    handle.enable_tracing().await;
    handle.clear_trace().await;
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");

    // When: software submits a copy, waits for async progress, and polls the completion.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let _snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    let trace_events = handle.trace_events().await;
    let trace_json = handle.trace_json().await;
    runner.request_shutdown().await;
    let _summary = runner.join().await.expect("runner joins cleanly");

    // Then: the trace stream records the full copy lifecycle and the destination bytes match.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::CommandSubmitted
            && event.operation() == Some(TraceOperation::Copy)
            && event.command_id() == Some(command_id)
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::DoorbellRunnable
            && event.operation().is_none()
            && event.command_id().is_none()
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::Scheduled
            && event.operation() == Some(TraceOperation::Copy)
            && event.command_id() == Some(command_id)
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::ExecutionSucceeded
            && event.operation() == Some(TraceOperation::Copy)
            && event.command_id() == Some(command_id)
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::CompletionPublished
            && event.operation().is_none()
            && event.command_id() == Some(command_id)
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::CompletionPolled
            && event.operation().is_none()
            && event.command_id() == Some(command_id)
    }));
    assert!(trace_json.contains("traceEvents"));
    assert!(trace_json.contains("\"ph\":\"i\""));
    assert!(trace_json.contains("\"ts\""));
    assert!(trace_json.contains("\"pid\""));
    assert!(trace_json.contains("\"tid\""));
    assert!(trace_json.contains("Copy"));
    assert!(trace_json.contains("Success"));
    assert!(trace_json.contains(&format!("{}", command_id.raw())));
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 4).await,
        Ok(vec![1, 2, 3, 4])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_records_error_event_without_success_completion() {
    // Given: a traced async runner and a copy that reads past the source buffer.
    let runner = AsyncDeviceRunner::spawn_native();
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    handle.enable_tracing().await;
    handle.clear_trace().await;
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;

    // When: software submits the faulting copy, waits for the error, and polls the event path.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
        .await
        .expect("faulting copy submission has ring space");
    let _snapshot = runner.wait_for_error_after(0).await;
    let event = driver
        .poll_event_on(&mut handle)
        .await
        .expect("event poll succeeds")
        .expect("fault event was published");
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists");
    let trace_events = handle.trace_events().await;
    let trace_json = handle.trace_json().await;
    runner.request_shutdown().await;
    let _summary = runner.join().await.expect("runner joins cleanly");

    // Then: the trace records the fault event path and no success completion.
    assert_eq!(event.command_id(), command_id);
    assert_eq!(completion, None);
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::ExecutionFaulted
            && event.operation() == Some(TraceOperation::Copy)
            && event.command_id() == Some(command_id)
            && event.detail() == "Error OutOfBounds"
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::EventPublished && event.command_id() == Some(command_id)
    }));
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::EventPolled && event.command_id() == Some(command_id)
    }));
    assert!(!trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::CompletionPublished
            && event.command_id() == Some(command_id)
    }));
    assert!(trace_json.contains("traceEvents"));
    assert!(trace_json.contains("Error"));
    assert!(trace_json.contains("OutOfBounds"));
}

#[tokio::test(flavor = "current_thread")]
async fn async_trace_log_records_stall_annotations_for_status_waits() {
    // Given: a traced async runner whose resources cannot satisfy a copy read.
    let runner = AsyncDeviceRunner::spawn_native_with_resource_config(
        ResourceConfig::default().with_read_slots(0),
    );
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    handle.enable_tracing().await;
    handle.clear_trace().await;
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;

    // When: software submits the copy and waits for the first stall annotation.
    driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let _snapshot = runner.wait_for_stall_after(0).await;
    let trace_events = handle.trace_events().await;
    let trace_json = handle.trace_json().await;
    runner.request_shutdown().await;
    let _summary = runner.join().await.expect("runner joins cleanly");

    // Then: the trace records execution-resource pressure as a stall event.
    assert!(trace_events.iter().any(|event: &TraceEvent| {
        event.kind() == TraceEventKind::Stall && event.detail() == "ExecutionResourcePressure"
    }));
    assert!(trace_json.contains("traceEvents"));
    assert!(trace_json.contains("Stall"));
    assert!(trace_json.contains("ExecutionResourcePressure"));
}
