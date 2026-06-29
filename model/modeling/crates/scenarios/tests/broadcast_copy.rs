#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, EventErrorKind, ModelError,
    desc::{BroadcastCopyRequest, BroadcastDestinationRange},
};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn copies_one_source_range_to_multiple_destinations_and_publishes_one_completion() {
    // Given: one source range and two destination ranges.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 2, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 8);
    let dst_a = driver.allocate_buffer(&mut device, 8);
    let dst_b = driver.allocate_buffer(&mut device, 8);
    driver
        .write_buffer(&mut device, src, &[0, 1, 2, 3, 4, 5, 6, 7])
        .expect("source write is in bounds");

    // When: software submits one broadcast copy command and the device executes it.
    let command_id = driver
        .submit_broadcast_copy(
            &mut device,
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
    let summary = device
        .run_until_idle(1)
        .expect("one step completes one broadcast command");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("broadcast published one completion");

    // Then: both destinations receive the same source bytes and only one completion appears.
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
    assert_eq!(
        driver.read_buffer(&device, dst_a, 8),
        Ok(vec![2, 3, 4, 5, 0, 0, 0, 0])
    );
    assert_eq!(
        driver.read_buffer(&device, dst_b, 8),
        Ok(vec![0, 0, 0, 2, 3, 4, 5, 0])
    );
}

#[test]
fn publishes_event_and_no_completion_when_destination_write_fails() {
    // Given: one valid destination followed by one too-small destination.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 2, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let valid_dst = driver.allocate_buffer(&mut device, 4);
    let bad_dst = driver.allocate_buffer(&mut device, 2);
    driver
        .write_buffer(&mut device, src, &[9, 8, 7, 6])
        .expect("source write is in bounds");

    // When: a broadcast command writes past the second destination.
    let command_id = driver
        .submit_broadcast_copy(
            &mut device,
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
    let result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("broadcast fault event was published");

    // Then: the original error is returned, one event is published, and no success completes.
    assert_eq!(
        result,
        Err(ModelError::OutOfBounds {
            buffer: bad_dst,
            len: 2,
            end: 4,
        })
    );
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), bad_dst);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 2, end: 4 }
    );
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
}
