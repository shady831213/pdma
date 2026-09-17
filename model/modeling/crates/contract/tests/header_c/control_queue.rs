use pdma_model_contract::{ControlQueue, ControlRingState};

#[test]
fn rust_nested_queue_matches_independent_c_fixture() -> super::TestResult {
    // Given: distinct command and response geometry with independent cursor values.
    let queue = ControlQueue {
        command: ControlRingState {
            base: 0x1000,
            capacity: 8,
            producer: 3,
            consumer: 1,
        },
        response: ControlRingState {
            base: 0x2000,
            capacity: 4,
            producer: 2,
            consumer: 1,
        },
    };
    // When: Rust encodes the nested queue descriptor.
    let bytes = queue.to_bytes()?;
    // Then: it matches the independent little-endian fixture including both padding spans.
    assert_eq!(bytes, super::QUEUE_BYTES);
    Ok(())
}
