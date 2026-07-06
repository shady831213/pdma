#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, EventErrorKind,
    desc::{BroadcastCopyRequest, BroadcastDestinationRange},
};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn copies_one_source_range_to_multiple_destinations_and_publishes_one_completion() {
    // Given: one source range and two destination ranges.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 2, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 8);
    let dst_a = driver.allocate_buffer_on(&mut handle, 8);
    let dst_b = driver.allocate_buffer_on(&mut handle, 8);
    driver
        .write_buffer_on(&mut handle, src, &[0, 1, 2, 3, 4, 5, 6, 7])
        .expect("source write is in bounds");

    // When: software submits one broadcast copy command and the device executes it.
    let command_id = driver
        .submit_broadcast_copy_on(
            &mut handle,
            BroadcastCopyRequest::new(
                src,
                2,
                4,
                vec![
                    BroadcastDestinationRange::new(dst_a, 0),
                    BroadcastDestinationRange::new(dst_b, 3),
                ],
            ),
        )
        .expect("broadcast submission has ring space");
    let snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("broadcast published one completion");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: both destinations receive the same source bytes and only one completion appears.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion_on(&mut handle), Ok(None));
    assert_eq!(
        driver.read_buffer_on(&handle, dst_a, 8),
        Ok(vec![2, 3, 4, 5, 0, 0, 0, 0])
    );
    assert_eq!(
        driver.read_buffer_on(&handle, dst_b, 8),
        Ok(vec![0, 0, 0, 2, 3, 4, 5, 0])
    );
}

#[test]
fn publishes_event_and_no_completion_when_destination_write_fails() {
    // Given: one valid destination followed by one too-small destination.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 2, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4);
    let valid_dst = driver.allocate_buffer_on(&mut handle, 4);
    let bad_dst = driver.allocate_buffer_on(&mut handle, 2);
    driver
        .write_buffer_on(&mut handle, src, &[9, 8, 7, 6])
        .expect("source write is in bounds");

    // When: a broadcast command writes past the second destination.
    let command_id = driver
        .submit_broadcast_copy_on(
            &mut handle,
            BroadcastCopyRequest::new(
                src,
                0,
                4,
                vec![
                    BroadcastDestinationRange::new(valid_dst, 0),
                    BroadcastDestinationRange::new(bad_dst, 0),
                ],
            ),
        )
        .expect("broadcast submission has ring space");
    let snapshot = runner.wait_for_error_after(0);
    let event = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("broadcast fault event was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the original error is returned, one event is published, and no success completes.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), bad_dst);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 2, end: 4 }
    );
    assert_eq!(driver.poll_completion_on(&mut handle), Ok(None));
}
