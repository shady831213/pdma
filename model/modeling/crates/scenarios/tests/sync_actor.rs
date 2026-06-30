#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ResourceConfig, desc::CopyRequest};
use pdma_model_device::{PdmaDevice, RunStatusSummary, StallReason, StepStatus};
use pdma_model_host::Driver;

#[test]
fn sync_actor_surface_interleaves_software_and_hardware_turns() {
    // Given: one device, one driver actor, and source/destination buffers.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup_on(&mut device, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut device, 4);
    let dst = driver.allocate_buffer_on(&mut device, 4);
    driver
        .write_buffer_on(&mut device, src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: software submits a copy, polls before hardware runs, then hardware gets one turn.
    let command_id = driver
        .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
        .expect("copy submission has ring space");
    let pre_step_completion = driver
        .poll_completion_on(&mut device)
        .expect("completion ring exists");
    let summary = device
        .run_until_idle(1)
        .expect("one hardware turn completes one copy");
    let completion = driver
        .poll_completion_on(&mut device)
        .expect("completion ring exists")
        .expect("completion was published");

    // Then: the actor-style surface preserves the copy outcome across interleaved turns.
    assert_eq!(pre_step_completion, None);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.read_buffer_on(&device, dst, 4), Ok(vec![1, 2, 3, 4]));
}

#[test]
fn sync_actor_surface_retries_hardware_after_software_polls_completion() {
    // Given: two software submissions behind a one-slot completion publication path.
    let mut device = PdmaDevice::with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let mut driver = Driver::setup_on(&mut device, 4, 4).expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut device, 4);
    let dst_a = driver.allocate_buffer_on(&mut device, 4);
    let src_b = driver.allocate_buffer_on(&mut device, 4);
    let dst_b = driver.allocate_buffer_on(&mut device, 4);
    driver
        .write_buffer_on(&mut device, src_a, &[10, 11, 12, 13])
        .expect("source A write is in bounds");
    driver
        .write_buffer_on(&mut device, src_b, &[20, 21, 22, 23])
        .expect("source B write is in bounds");

    // When: hardware stalls on the second completion until software polls the first one.
    let first_command = driver
        .submit_copy_on(&mut device, CopyRequest::new(src_a, dst_a, 4))
        .expect("first copy submission has ring space");
    let second_command = driver
        .submit_copy_on(&mut device, CopyRequest::new(src_b, dst_b, 4))
        .expect("second copy submission has ring space");
    let first_status = device.step_with_status();
    let stalled = device.run_until_blocked(1);
    let first_completion = driver
        .poll_completion_on(&mut device)
        .expect("completion ring exists")
        .expect("first completion was published");
    let second_summary = device
        .run_until_idle(1)
        .expect("second copy retires after software polling");
    let second_completion = driver
        .poll_completion_on(&mut device)
        .expect("completion ring exists")
        .expect("second completion was published");

    // Then: software polling is the turn that lets hardware publish the next completion.
    assert_eq!(first_status, Ok(StepStatus::Progressed));
    assert_eq!(
        stalled,
        Ok(RunStatusSummary {
            steps: 0,
            terminal_status: StepStatus::Stalled(StallReason::CompletionPublicationPressure),
        })
    );
    assert_eq!(first_completion.command_id(), first_command);
    assert_eq!(first_completion.status(), CompletionStatus::Success);
    assert_eq!(second_summary.steps, 1);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_completion.status(), CompletionStatus::Success);
    assert_eq!(
        driver.read_buffer_on(&device, dst_a, 4),
        Ok(vec![10, 11, 12, 13])
    );
    assert_eq!(
        driver.read_buffer_on(&device, dst_b, 4),
        Ok(vec![20, 21, 22, 23])
    );
    assert_eq!(driver.poll_completion_on(&mut device), Ok(None));
}
