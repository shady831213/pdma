#![allow(missing_docs)]

use pdma_modeling::desc::{CopyRequest, VectorAddRequest, VectorBufferRange};
use pdma_modeling::{
    Driver, ModelError, PdmaDevice, ResourceConfig, RunStatusSummary, StallReason, StepStatus,
};

fn setup(config: ResourceConfig) -> Result<(PdmaDevice, Driver), ModelError> {
    let mut device = PdmaDevice::with_resource_config(config);
    let driver = Driver::setup(&mut device, 4, 4)?;
    Ok((device, driver))
}

#[test]
fn idle_step_reports_no_work_status() {
    // Given: an idle device with no queued commands.
    let (mut device, _driver) = setup(ResourceConfig::default()).expect("queue setup is valid");

    // When: software asks for a status-annotated step.
    let status = device.step_with_status();

    // Then: the device reports an idle/no-work status.
    assert!(matches!(status, Ok(StepStatus::Idle)));
}

#[test]
fn execution_resource_pressure_reports_a_stall_reason() {
    // Given: a vector add that needs compute resources on a device with none available.
    let (mut device, mut driver) =
        setup(ResourceConfig::default().with_compute_slots(0)).expect("queue setup is valid");
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

    // When: software steps the device with stall annotations enabled.
    let status = device.step_with_status();

    // Then: the device reports an execution-resource stall.
    assert!(matches!(
        status,
        Ok(StepStatus::Stalled(StallReason::ExecutionResourcePressure))
    ));
}

#[test]
fn completion_publication_pressure_preserves_order_in_status_annotations() {
    // Given: two copies behind a one-slot completion publication path.
    let (mut device, mut driver) =
        setup(ResourceConfig::default().with_completion_publication_slots(1))
            .expect("queue setup is valid");
    let src_a = driver.allocate_buffer(&mut device, 4);
    let dst_a = driver.allocate_buffer(&mut device, 4);
    let src_b = driver.allocate_buffer(&mut device, 4);
    let dst_b = driver.allocate_buffer(&mut device, 4);
    driver
        .write_buffer(&mut device, src_a, &[1, 2, 3, 4])
        .expect("source A write is in bounds");
    driver
        .write_buffer(&mut device, src_b, &[5, 6, 7, 8])
        .expect("source B write is in bounds");
    driver
        .submit_copy(&mut device, CopyRequest::new(src_a, dst_a, 4))
        .expect("first copy submission has ring space");
    driver
        .submit_copy(&mut device, CopyRequest::new(src_b, dst_b, 4))
        .expect("second copy submission has ring space");

    // When: software steps once, then asks for a run-until-blocked summary.
    let first_status = device.step_with_status();
    let summary = device.run_until_blocked(1);

    // Then: the first retirement is successful and the blocked summary preserves the pending order.
    assert_eq!(first_status, Ok(StepStatus::Progressed));
    assert_eq!(
        summary,
        Ok(RunStatusSummary {
            steps: 0,
            terminal_status: StepStatus::Stalled(StallReason::CompletionPublicationPressure),
        })
    );
}

#[test]
fn event_publication_pressure_reports_new_fault_after_event_path_fills() {
    // Given: two faulting copies behind a one-deep event publication path.
    let (mut device, mut driver) = setup(ResourceConfig::default().with_event_publication_slots(1))
        .expect("queue setup is valid");
    let bad_src_a = driver.allocate_buffer(&mut device, 4);
    let bad_dst_a = driver.allocate_buffer(&mut device, 8);
    let bad_src_b = driver.allocate_buffer(&mut device, 4);
    let bad_dst_b = driver.allocate_buffer(&mut device, 8);
    driver
        .submit_copy(&mut device, CopyRequest::new(bad_src_a, bad_dst_a, 8))
        .expect("first faulting copy has ring space");
    driver
        .submit_copy(&mut device, CopyRequest::new(bad_src_b, bad_dst_b, 8))
        .expect("second faulting copy has ring space");

    // When: the first fault publishes its event, then the second fault reaches a full event path.
    let first_status = device.step_with_status();
    let second_status = device.step_with_status();

    // Then: the first fault preserves its original error and the second reports publication pressure.
    assert!(matches!(first_status, Err(ModelError::OutOfBounds { .. })));
    assert_eq!(
        second_status,
        Ok(StepStatus::Stalled(StallReason::EventPublicationPressure))
    );
}

#[test]
fn fault_retirement_pending_retry_reports_retry_stall_reason() {
    // Given: a faulting copy that cannot retire because the event path is full.
    let (mut device, mut driver) = setup(ResourceConfig::default().with_event_publication_slots(0))
        .expect("queue setup is valid");
    let bad_src = driver.allocate_buffer(&mut device, 4);
    let bad_dst = driver.allocate_buffer(&mut device, 8);
    driver
        .submit_copy(&mut device, CopyRequest::new(bad_src, bad_dst, 8))
        .expect("faulting copy has ring space");

    // When: software hits event pressure on the new fault and retries while it is still pending.
    let first_status = device.step_with_status();
    let retry_status = device.step_with_status();

    // Then: the retry is distinguished from the first event-publication stall.
    assert_eq!(
        first_status,
        Ok(StepStatus::Stalled(StallReason::EventPublicationPressure))
    );
    assert_eq!(
        retry_status,
        Ok(StepStatus::Stalled(
            StallReason::FaultRetirementPendingRetry
        ))
    );
}

#[test]
fn run_until_blocked_returns_terminal_status_without_losing_idle_errors() {
    // Given: a device with one copy that cannot acquire execution resources.
    let (mut device, mut driver) =
        setup(ResourceConfig::default().with_read_slots(0)).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 4);
    driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 4))
        .expect("copy submission has ring space");

    // When: software runs until blocked and then checks the legacy idle API.
    let blocked = device.run_until_blocked(1);
    let idle = device.run_until_idle(1);

    // Then: the blocked API names the stall while the legacy idle API still surfaces step-budget exhaustion.
    assert_eq!(
        blocked,
        Ok(RunStatusSummary {
            steps: 0,
            terminal_status: StepStatus::Stalled(StallReason::ExecutionResourcePressure),
        })
    );
    assert_eq!(idle, Err(ModelError::StepBudgetExhausted { max_steps: 1 }));
}
