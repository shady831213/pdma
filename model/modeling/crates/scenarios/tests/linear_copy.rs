#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ModelError, desc::CopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn copies_bytes_end_to_end_when_driver_polls_completion() {
    // Given: a device, one driver queue, and source/destination buffers.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 5);
    let dst = driver.allocate_buffer(&mut device, 5);
    driver
        .write_buffer(&mut device, src, &[1, 2, 3, 4, 5])
        .expect("source write is in bounds");

    // When: software submits a copy and the device runs until idle.
    let command_id = driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 5))
        .expect("copy submission has ring space");
    let summary = device
        .run_until_idle(1)
        .expect("one step completes one copy");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("completion was published");

    // Then: completion metadata routes back to the command and destination equals source.
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.read_buffer(&device, dst, 5), Ok(vec![1, 2, 3, 4, 5]));
}

#[test]
fn reports_step_budget_exhaustion_when_work_remains() {
    // Given: one pending copy command.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 1);
    let dst = driver.allocate_buffer(&mut device, 1);
    driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 1))
        .expect("copy submission has ring space");

    // When: the device is given no steps.
    let result = device.run_until_idle(0);

    // Then: the pending command is reported as budget exhaustion.
    assert_eq!(
        result,
        Err(ModelError::StepBudgetExhausted { max_steps: 0 })
    );
}
