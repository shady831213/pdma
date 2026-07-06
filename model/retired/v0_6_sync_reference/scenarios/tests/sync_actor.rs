#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ResourceConfig, desc::CopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn sync_runner_surface_interleaves_software_and_hardware_turns() {
    // Given: a device runner, a runner handle, and source/destination buffers.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4);
    let dst = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: software submits a copy, waits on the runner, and then polls the completion.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the runner-backed surface preserves the copy outcome across interleaved turns.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(driver.read_buffer_on(&handle, dst, 4), Ok(vec![1, 2, 3, 4]));
}

#[test]
fn sync_runner_surface_retries_after_completion_publication_pressure() {
    // Given: two software submissions behind a one-slot completion publication path.
    let device = PdmaDevice::with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 4);
    let dst_a = driver.allocate_buffer_on(&mut handle, 4);
    let src_b = driver.allocate_buffer_on(&mut handle, 4);
    let dst_b = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, src_a, &[10, 11, 12, 13])
        .expect("source A write is in bounds");
    driver
        .write_buffer_on(&mut handle, src_b, &[20, 21, 22, 23])
        .expect("source B write is in bounds");

    // When: hardware stalls on the second completion until software polls the first one.
    let first_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
        .expect("first copy submission has ring space");
    let second_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
        .expect("second copy submission has ring space");
    let first_snapshot = runner.wait_for_progress_after(0);
    let first_completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("first completion was published");
    let second_snapshot = runner.wait_for_progress_after(first_snapshot.steps);
    let second_completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("second completion was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: software polling is the turn that lets hardware publish the next completion.
    assert_eq!(first_completion.command_id(), first_command);
    assert_eq!(first_completion.status(), CompletionStatus::Success);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_completion.status(), CompletionStatus::Success);
    assert_eq!(first_snapshot.steps, 1);
    assert_eq!(second_snapshot.steps, 2);
    assert_eq!(summary.steps, 2);
    assert_eq!(
        driver.read_buffer_on(&handle, dst_a, 4),
        Ok(vec![10, 11, 12, 13])
    );
    assert_eq!(
        driver.read_buffer_on(&handle, dst_b, 4),
        Ok(vec![20, 21, 22, 23])
    );
}

#[test]
fn sync_runner_surface_records_errors_without_stopping_the_loop() {
    // Given: a runner-backed device, a driver handle, and one faulting command.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let bad_src = driver.allocate_buffer_on(&mut handle, 4);
    let bad_dst = driver.allocate_buffer_on(&mut handle, 4);
    let good_src = driver.allocate_buffer_on(&mut handle, 4);
    let good_dst = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, good_src, &[7, 8, 9, 10])
        .expect("good source write is in bounds");

    // When: a bad command faults and a later good command still runs.
    let bad_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(bad_src, bad_dst, 8))
        .expect("faulting copy submission has ring space");
    let good_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 4))
        .expect("good copy submission has ring space");
    let error_snapshot = runner.wait_for_error_after(0);
    let fault = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("fault event was published");
    let progress_snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("good completion was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the fault is recorded and the loop still retires the later good command.
    assert_eq!(fault.command_id(), bad_command);
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(progress_snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(
        driver.read_buffer_on(&handle, good_dst, 4),
        Ok(vec![7, 8, 9, 10])
    );
}

#[test]
fn sync_runner_surface_reports_a_shutdown_and_join_summary() {
    // Given: a runner-backed device and one completed command.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4);
    let dst = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, src, &[1, 3, 5, 7])
        .expect("source write is in bounds");

    // When: software submits a copy, waits once, and then shuts the runner down.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: shutdown produces a join summary after the successful copy.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(driver.read_buffer_on(&handle, dst, 4), Ok(vec![1, 3, 5, 7]));
}
