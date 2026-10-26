//! Generated PDMA wire-codec behavior.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::Event;
use pdma_model_contract::dma::{ErrorInfo, EventInfoValues};

#[test]
fn event_round_trips_when_code_is_assigned() -> Result<(), SchemaCodecError> {
    let progress = ErrorInfo {
        bytes_completed: 0x0102_0304_0506_0708,
    };
    for (info, discriminant) in [
        (EventInfoValues::InvalidLength(progress), 1),
        (EventInfoValues::ReadError(progress), 2),
        (EventInfoValues::WriteError(progress), 3),
        (EventInfoValues::LengthMismatch(progress), 4),
        (EventInfoValues::ListReadError(progress), 5),
        (EventInfoValues::Unsupported(progress), 6),
        (EventInfoValues::InvalidOperation(progress), 7),
        (EventInfoValues::ArithmeticOverflow(progress), 8),
    ] {
        // Given: each assigned event code and a visible command ID.
        let event = Event {
            id: 7,
            code: info.tag(),
            info,
        };

        // When: the generated codec encodes the event.
        let bytes = event.to_bytes()?;

        // Then: the wire code and decoded value agree.
        let mut expected = [0; 24];
        expected[0] = 7;
        expected[8] = discriminant;
        expected[16..24].copy_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
        assert_eq!(bytes, expected);
        assert_eq!(Event::from_bytes(&bytes)?, event);
        assert_eq!(Event::SPEC.alignment(), 8);
    }
    Ok(())
}

#[test]
fn event_rejects_reserved_code_when_discriminant_is_unassigned() {
    // Given: an event containing the invalid zero code.
    let mut bytes = [0; Event::RECORD_SIZE];
    bytes[0] = 7;
    bytes[8] = 0;

    // When: raw storage crosses the generated typed boundary.
    let result = Event::from_bytes(&bytes);

    // Then: the error identifies the code field and observed discriminant.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: Event::CODE_IDENTITY.at_byte(8),
            observed: 0,
        })
    );
}
