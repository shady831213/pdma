#![allow(missing_docs)]

use pdma_modeling::desc::{VectorAddRequest, VectorBufferRange};
use pdma_modeling::{CompletionStatus, Driver, EventErrorKind, ModelError, PdmaDevice};

#[test]
fn adds_bytes_with_wraparound_and_publishes_one_completion() {
    // Given: two source buffers with non-zero offsets, a sentinel-filled destination, and one vector-add request.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src_a = driver.allocate_buffer(&mut device, 12);
    let src_b = driver.allocate_buffer(&mut device, 12);
    let dst = driver.allocate_buffer(&mut device, 16);
    device
        .write_buffer(
            src_a,
            0,
            &[
                0xA0, 0xA1, 1, 250, 0, 127, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
            ],
        )
        .expect("source A write is in bounds");
    device
        .write_buffer(
            src_b,
            0,
            &[
                0xB0, 0xB1, 2, 10, 255, 1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7,
            ],
        )
        .expect("source B write is in bounds");
    device
        .write_buffer(dst, 0, &[0xCC; 16])
        .expect("destination write is in bounds");

    // When: software submits one vector add command and the device executes it.
    let command_id = driver
        .submit_vector_add(
            &mut device,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 2),
                VectorBufferRange::new(src_b, 2),
                VectorBufferRange::new(dst, 6),
                4,
            ),
        )
        .expect("vector-add submission has ring space");
    let summary = device
        .run_until_idle(1)
        .expect("one step completes one vector-add command");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("vector-add published one completion");

    // Then: byte-wise wrapping addition lands at the destination offset and only one success completion appears.
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
    assert_eq!(
        device.read_buffer(dst, 0, 16),
        Ok(vec![
            0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 3, 4, 255, 128, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
        ])
    );
}

#[test]
fn publishes_event_and_no_completion_when_source_b_read_fails() {
    // Given: a vector add whose second source range runs past the source B buffer.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src_a = driver.allocate_buffer(&mut device, 8);
    let src_b = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 8);

    // When: the invalid vector add is submitted and executed.
    let command_id = driver
        .submit_vector_add(
            &mut device,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 1),
                VectorBufferRange::new(src_b, 1),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .expect("vector-add submission has ring space");
    let result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("source-B fault event was published");

    // Then: the model returns the original out-of-bounds error, publishes one event, and does not publish success.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(
        result,
        Err(ModelError::OutOfBounds {
            buffer: src_b,
            len: 4,
            end: 5,
        })
    );
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src_b);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 5 }
    );
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
}

#[test]
fn publishes_event_and_no_completion_when_destination_write_fails() {
    // Given: a vector add whose destination range runs past the destination buffer.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src_a = driver.allocate_buffer(&mut device, 8);
    let src_b = driver.allocate_buffer(&mut device, 8);
    let dst = driver.allocate_buffer(&mut device, 4);
    device
        .write_buffer(src_a, 0, &[0xD0, 0xD1, 9, 8, 7, 6, 0xD2, 0xD3])
        .expect("source A write is in bounds");
    device
        .write_buffer(src_b, 0, &[0xE0, 0xE1, 1, 2, 3, 4, 0xE2, 0xE3])
        .expect("source B write is in bounds");

    // When: the vector add tries to write four bytes starting before the end of the destination.
    let command_id = driver
        .submit_vector_add(
            &mut device,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 2),
                VectorBufferRange::new(src_b, 2),
                VectorBufferRange::new(dst, 1),
                4,
            ),
        )
        .expect("vector-add submission has ring space");
    let result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("destination fault event was published");

    // Then: the model returns the destination bounds error, publishes one event, and does not publish success.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(
        result,
        Err(ModelError::OutOfBounds {
            buffer: dst,
            len: 4,
            end: 5,
        })
    );
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), dst);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 5 }
    );
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
}
