#![allow(missing_docs)]

use pdma_modeling::desc::CopyRequest;
use pdma_modeling::{
    CompletionStatus, Driver, ModelError, PdmaDevice, ResourceConfig, StallReason, StepStatus,
    TraceEvent, TraceEventKind, TraceOperation, TraceSink, VectorAddRequest, VectorBufferRange,
};

fn setup(config: ResourceConfig) -> Result<(PdmaDevice, Driver), ModelError> {
    let mut device = PdmaDevice::with_resource_config(config);
    let driver = Driver::setup(&mut device, 4, 4)?;
    Ok((device, driver))
}

fn traced_setup(config: ResourceConfig) -> Result<(PdmaDevice, Driver), ModelError> {
    let (mut device, driver) = setup(config)?;
    let _ = device.enable_tracing();
    device.clear_trace();
    Ok((device, driver))
}

const fn assert_trace_api_types_exist() {
    let _ = core::mem::size_of::<TraceEvent>();
    let _ = core::mem::size_of::<TraceEventKind>();
    let _ = core::mem::size_of::<TraceOperation>();
    let _ = core::mem::size_of::<TraceSink>();
}

fn assert_trace_json_shape(json: &str) {
    assert!(json.contains("traceEvents"));
    assert!(json.contains("\"ph\":\"i\""));
    assert!(json.contains("\"ts\""));
    assert!(json.contains("\"pid\""));
    assert!(json.contains("\"tid\""));
}

#[test]
fn trace_log_records_complete_copy_lifecycle() {
    // Given: tracing is enabled for a simple linear copy.
    assert_trace_api_types_exist();
    let (mut device, mut driver) =
        traced_setup(ResourceConfig::default()).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 4);
    driver
        .write_buffer(&mut device, src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: the copy is submitted, executed, and its completion is polled.
    let command_id = driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 4))
        .expect("copy submission has ring space");
    let summary = device
        .run_until_idle(1)
        .expect("one step completes one copy");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("copy completion was published");
    let trace_events = device.trace_events();
    let trace_json = device.trace_json();

    // Then: the structured log and JSON trace both describe the full copy lifecycle.
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        trace_events.first().map(TraceEvent::kind),
        Some(TraceEventKind::CommandSubmitted)
    );
    assert_eq!(
        trace_events.first().and_then(TraceEvent::operation),
        Some(TraceOperation::Copy)
    );
    assert_eq!(
        trace_events.first().and_then(TraceEvent::command_id),
        Some(command_id)
    );
    assert!(
        trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::ExecutionSucceeded
                && event.command_id() == Some(command_id)
                && event.operation() == Some(TraceOperation::Copy))
    );
    assert!(
        trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::CompletionPublished
                && event.command_id() == Some(command_id)
                && event.detail() == "status Success")
    );
    assert_trace_json_shape(&trace_json);
    assert!(trace_json.contains("Copy"));
    assert!(trace_json.contains("Success"));
    assert!(trace_json.contains(&format!("{}", command_id.raw())));
    assert_eq!(driver.read_buffer(&device, dst, 4), Ok(vec![1, 2, 3, 4]));
}

#[test]
fn chrome_trace_json_exports_complete_copy_lifecycle() {
    // Given: tracing is enabled for a copy with an explicit queue-local lifecycle.
    let (mut device, mut driver) =
        traced_setup(ResourceConfig::default()).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 4);
    driver
        .write_buffer(&mut device, src, &[9, 8, 7, 6])
        .expect("source write is in bounds");

    // When: software submits and retires the copy.
    let command_id = driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 4))
        .expect("copy submission has ring space");
    let _ = device.run_until_idle(1).expect("copy retires successfully");
    let _ = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("copy completion was published");
    let trace_json = device.trace_json();

    // Then: the JSON is Chrome/Perfetto-shaped and names the copy lifecycle data.
    assert_trace_json_shape(&trace_json);
    assert!(trace_json.contains("Copy"));
    assert!(trace_json.contains("status"));
    assert!(trace_json.contains("id"));
    assert!(trace_json.contains(&format!("{}", command_id.raw())));
}

#[test]
fn trace_log_records_error_event_without_success_completion() {
    // Given: tracing is enabled for an out-of-bounds copy.
    let (mut device, mut driver) =
        traced_setup(ResourceConfig::default()).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 8);

    // When: the invalid copy is executed.
    let command_id = driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 8))
        .expect("copy submission has ring space");
    let result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("error event was published");
    let completion_poll = driver.poll_completion(&mut device);
    let trace_events = device.trace_events();
    let trace_json = device.trace_json();

    // Then: the model records the error path and does not publish a success completion.
    assert!(matches!(result, Err(ModelError::OutOfBounds { .. })));
    assert_eq!(event.command_id(), command_id);
    assert_eq!(completion_poll, Ok(None));
    assert!(
        trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::ExecutionFaulted
                && event.command_id() == Some(command_id)
                && event.detail() == "Error OutOfBounds")
    );
    assert_trace_json_shape(&trace_json);
    assert!(trace_json.contains("Error"));
    assert!(trace_json.contains("OutOfBounds"));
    assert!(trace_json.contains(&format!("{}", command_id.raw())));
}

#[test]
fn trace_log_records_stall_annotations_for_status_steps() {
    // Given: a traced device that cannot acquire compute resources.
    let (mut device, mut driver) = traced_setup(ResourceConfig::default().with_compute_slots(0))
        .expect("queue setup is valid");
    let src_a = driver.allocate_buffer(&mut device, 8);
    let src_b = driver.allocate_buffer(&mut device, 8);
    let dst = driver.allocate_buffer(&mut device, 8);
    driver
        .submit_vector_add(
            &mut device,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 0),
                VectorBufferRange::new(src_b, 0),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .expect("vector-add submission has ring space");

    // When: software asks for a stall-annotated step.
    let status = device.step_with_status();
    let trace_events = device.trace_events();
    let trace_json = device.trace_json();

    // Then: the stall is preserved in both the structured log and the exported JSON.
    assert!(matches!(
        status,
        Ok(StepStatus::Stalled(StallReason::ExecutionResourcePressure))
    ));
    assert!(
        trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::Stall
                && event.detail() == "ExecutionResourcePressure")
    );
    assert_trace_json_shape(&trace_json);
    assert!(trace_json.contains("Stall"));
    assert!(trace_json.contains("ExecutionResourcePressure"));
}

#[test]
fn tracing_preserves_step_and_run_until_idle_behavior() {
    // Given: the same copy on a baseline device and a traced device.
    let (mut baseline_device, mut baseline_driver) =
        setup(ResourceConfig::default()).expect("queue setup is valid");
    let baseline_src = baseline_driver.allocate_buffer(&mut baseline_device, 4);
    let baseline_dst = baseline_driver.allocate_buffer(&mut baseline_device, 4);
    baseline_driver
        .write_buffer(&mut baseline_device, baseline_src, &[4, 3, 2, 1])
        .expect("source write is in bounds");

    let (mut traced_device, mut traced_driver) =
        traced_setup(ResourceConfig::default()).expect("queue setup is valid");
    let traced_src = traced_driver.allocate_buffer(&mut traced_device, 4);
    let traced_dst = traced_driver.allocate_buffer(&mut traced_device, 4);
    traced_driver
        .write_buffer(&mut traced_device, traced_src, &[4, 3, 2, 1])
        .expect("source write is in bounds");

    // When: both devices run the same scenario.
    baseline_driver
        .submit_copy(
            &mut baseline_device,
            CopyRequest::new(baseline_src, baseline_dst, 4),
        )
        .expect("baseline copy submission has ring space");
    traced_driver
        .submit_copy(
            &mut traced_device,
            CopyRequest::new(traced_src, traced_dst, 4),
        )
        .expect("traced copy submission has ring space");
    let baseline_status = baseline_device.step();
    let traced_summary = traced_device
        .run_until_idle(1)
        .expect("traced device still runs to idle");
    let baseline_completion = baseline_driver
        .poll_completion(&mut baseline_device)
        .expect("completion ring exists")
        .expect("baseline copy completion was published");
    let traced_completion = traced_driver
        .poll_completion(&mut traced_device)
        .expect("completion ring exists")
        .expect("traced copy completion was published");

    // Then: tracing does not change step/run-until-idle behavior or the completed data path.
    assert_eq!(baseline_status, Ok(true));
    assert_eq!(traced_summary.steps, 1);
    assert_eq!(baseline_completion.status(), CompletionStatus::Success);
    assert_eq!(traced_completion.status(), CompletionStatus::Success);
    assert_eq!(
        baseline_driver.read_buffer(&baseline_device, baseline_dst, 4),
        Ok(vec![4, 3, 2, 1])
    );
    assert_eq!(
        traced_driver.read_buffer(&traced_device, traced_dst, 4),
        Ok(vec![4, 3, 2, 1])
    );
}
