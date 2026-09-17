//! Control-channel wire and register layout boundary tests.

use pdma_model_contract::{
    ControlRegisters, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
    ControlRingRegisters,
};

#[test]
fn request_encodes_little_endian_and_rejects_unknown_opcode() {
    // Given: a release request with distinguishable fields.
    let request = ControlRequest {
        id: 0x0102_0304_0506_0708,
        opcode: ControlRequestOpcode::ReleaseChannel,
        channel: 0x1122_3344,
    };
    // When: it crosses the wire boundary.
    let mut bytes = request.to_bytes().unwrap();
    // Then: its fixed layout is exact and unknown operations are rejected.
    assert_eq!(
        bytes,
        [8, 7, 6, 5, 4, 3, 2, 1, 1, 0, 0, 0, 0x44, 0x33, 0x22, 0x11]
    );
    assert_eq!(ControlRequest::from_bytes(&bytes).unwrap(), request);
    assert_eq!(ControlRequest::SPEC.alignment(), 8);
    bytes[8] = 2;
    assert!(ControlRequest::from_bytes(&bytes).is_err());
}

#[test]
fn response_preserves_each_assigned_status() {
    // Given: every assigned firmware outcome.
    for (status, raw) in [
        (ControlResponseStatus::Success, 0),
        (ControlResponseStatus::NoResources, 1),
        (ControlResponseStatus::InvalidChannel, 2),
        (ControlResponseStatus::NotAllocated, 3),
    ] {
        let response = ControlResponse {
            id: 9,
            status,
            channel: 2,
        };
        // When: the response is encoded.
        let bytes = response.to_bytes().unwrap();
        // Then: the assigned tag and complete response survive the wire.
        assert_eq!(bytes, [9, 0, 0, 0, 0, 0, 0, 0, raw, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(ControlResponse::from_bytes(&bytes).unwrap(), response);
        assert_eq!(ControlResponse::SPEC.alignment(), 8);
    }
}

#[test]
fn register_composition_has_independent_command_response_and_enable() {
    // Given/When: the generated layout composes two ring maps and enable.
    // Then: each leaf has its independent address and expected extent.
    assert_eq!(ControlRegisters::COMMAND.offset(), 0);
    assert_eq!(ControlRegisters::RESPONSE.offset(), 24);
    assert_eq!(ControlRegisters::ENABLE.offset(), 44);
    assert_eq!(ControlRegisters::EXTENT, 48);
    assert_eq!(ControlRingRegisters::BASE.offset(), 0);
    assert_eq!(ControlRingRegisters::CAPACITY.offset(), 8);
    assert_eq!(ControlRingRegisters::PRODUCER.offset(), 12);
    assert_eq!(ControlRingRegisters::CONSUMER.offset(), 16);
}
