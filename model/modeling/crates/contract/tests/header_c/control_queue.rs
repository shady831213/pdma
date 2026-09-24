use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
    ControlRingState,
};

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

#[test]
fn rust_control_records_match_c_consumer_fixtures() -> super::TestResult {
    // Given: independent wire bytes for acquisition, release and their responses.
    // When: the Rust contract encodes the same records used by the C consumer.
    let control_records = [
        ControlRequest {
            context: 0,
            attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
            id: 0x0807_0605_0403_0201,
            function: 7,
            opcode: ControlRequestOpcode::ReleaseChannel,
            channel: 0x1020_3040,
        }
        .to_bytes()?
        .to_vec(),
        ControlRequest {
            context: 0,
            attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
            id: 9,
            function: 2,
            opcode: ControlRequestOpcode::AcquireChannel,
            channel: 0,
        }
        .to_bytes()?
        .to_vec(),
        ControlResponse {
            context: 0,
            id: 9,
            status: ControlResponseStatus::Success,
            channel: 2,
        }
        .to_bytes()?
        .to_vec(),
        ControlResponse {
            context: 0,
            id: 0x0807_0605_0403_0201,
            status: ControlResponseStatus::NotAllocated,
            channel: 0x1020_3040,
        }
        .to_bytes()?
        .to_vec(),
    ];
    // Then: Rust matches the independently specified C acceptance fixtures.
    assert_eq!(control_records.concat(), super::CONTROL_BYTES.concat());
    Ok(())
}
