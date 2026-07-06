#![allow(missing_docs)]

use pdma_model_contract::{ModelError, ResourceConfig, TraceEventKind, desc::CopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

fn assert_trace_json_shape(json: &str) {
    assert!(json.contains("traceEvents"));
    assert!(json.contains("\"ph\":\"i\""));
    assert!(json.contains("\"ts\""));
    assert!(json.contains("\"pid\""));
    assert!(json.contains("\"tid\""));
}

#[test]
fn trace_log_records_polling_without_consuming_unrelated_queue_events() {
    // Given: two traced drivers where one queue faults and the other queue remains unrelated.
    let mut device = PdmaDevice::with_resource_config(ResourceConfig::default());
    let mut driver_a = Driver::setup_on(&mut device, 1, 4).expect("first queue setup is valid");
    let mut driver_b = Driver::setup_on(&mut device, 1, 4).expect("second queue setup is valid");
    let _ = device.enable_tracing();
    device.clear_trace();
    let a_src = driver_a.allocate_buffer_on(&mut device, 4);
    let a_dst = driver_a.allocate_buffer_on(&mut device, 8);
    let b_src = driver_b.allocate_buffer_on(&mut device, 1);
    let b_dst = driver_b.allocate_buffer_on(&mut device, 1);
    driver_a
        .submit_copy_on(&mut device, CopyRequest::new(a_src, a_dst, 8))
        .expect("faulting copy has ring space");

    // When: the fault is published and an unrelated queue polls first.
    assert!(matches!(
        device.run_until_idle(1),
        Err(ModelError::OutOfBounds { .. })
    ));
    let wrong_driver_event = driver_b
        .poll_event_on(&mut device)
        .expect("event ring exists");
    let owner_event = driver_a
        .poll_event_on(&mut device)
        .expect("event ring exists")
        .expect("owning driver can still poll its event");
    let submit_after_unrelated_poll =
        driver_b.submit_copy_on(&mut device, CopyRequest::new(b_src, b_dst, 1));
    let trace_events = device.trace_events();
    let trace_json = device.trace_json();

    // Then: the unrelated poll does not consume the event and the trace records the polling path.
    assert_eq!(wrong_driver_event, None);
    assert_eq!(owner_event.queue_id(), driver_a.queue_id());
    assert!(submit_after_unrelated_poll.is_ok());
    assert!(
        trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::EventPolled
                && event.queue_id() == Some(driver_b.queue_id())
                && event.detail() == "event poll None")
    );
    assert!(
        trace_events
            .iter()
            .any(|event| event.kind() == TraceEventKind::EventPolled
                && event.queue_id() == Some(driver_a.queue_id())
                && event.detail() == "event poll Some")
    );
    assert_trace_json_shape(&trace_json);
    assert!(trace_json.contains("poll"));
}
