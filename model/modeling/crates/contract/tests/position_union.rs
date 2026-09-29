//! Typed continuation positions at the wire boundary.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{Endpoint, EndpointPayloadValues, Position, Response};

#[test]
fn response_rejects_unknown_continuation_modes() -> Result<(), Box<dyn std::error::Error>> {
    for offset in [33, 34] {
        // Given: an otherwise valid response with an unknown source or destination position mode.
        let mut bytes = [0; Response::RECORD_SIZE];
        bytes[8] = 6;
        bytes[32] = 1;
        *bytes.get_mut(offset).ok_or("position tag offset")? = 2;
        // When: software decodes the hardware response.
        let result = Response::from_bytes(&bytes);
        // Then: neither position can silently reinterpret an unknown mode as an index or offset.
        assert!(
            matches!(
                result,
                Err(SchemaCodecError::UnknownTag { observed: 2, .. })
            ),
            "{result:?}"
        );
    }
    Ok(())
}

#[test]
fn positions_round_trip_full_width_and_reject_reserved_words()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both position variants use values that do not fit narrower fields.
    let positions = [
        Position::linear(u64::MAX),
        Position::scatter_gather(u64::MAX - 1, u64::MAX),
    ];
    for position in positions {
        // When: the generated codec encodes and decodes the wire position.
        let bytes = position.to_bytes()?;
        // Then: the variant and every payload bit survive.
        assert_eq!(Position::RECORD_SIZE, 24);
        assert_eq!(Position::from_bytes(&bytes)?, position);
    }
    // Given: a Linear response position with data in its unused SG-only word.
    for offset in [48, 64] {
        let mut bytes = [0; Response::RECORD_SIZE];
        bytes[8] = 6;
        bytes[32] = 1;
        *bytes.get_mut(offset).ok_or("reserved position word")? = 1;
        // When / Then: decoding rejects it instead of interpreting a spurious index.
        assert!(matches!(
            Response::from_bytes(&bytes),
            Err(SchemaCodecError::NonZeroReserved { observed: 1, .. })
        ));
    }
    Ok(())
}

#[test]
fn continuation_preserves_geometry_and_rejects_cross_mode_positions()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: independent Linear and SG endpoints with non-default contexts.
    let linear = Endpoint {
        context: 7,
        ..Endpoint::linear(0x1000, 3)
    };
    let sg = Endpoint {
        context: 9,
        ..Endpoint::scatter_gather(0x2000, 8)
    };
    // When: software applies the two self-describing continuation positions.
    let next_linear = linear
        .with_start(Position::linear(40))
        .ok_or("linear continuation")?;
    let next_sg = sg
        .with_start(Position::scatter_gather(2, 5))
        .ok_or("SG continuation")?;
    // Then: only the position changes, and the opposite variant cannot be applied.
    assert_eq!(next_linear.context, 7);
    assert_eq!(next_sg.context, 9);
    assert_eq!(next_linear.start(), Position::linear(40));
    assert_eq!(next_sg.start(), Position::scatter_gather(2, 5));
    assert!(
        matches!(next_linear.payload, EndpointPayloadValues::Linear(value) if value.address == 0x1000)
    );
    assert!(
        matches!(next_sg.payload, EndpointPayloadValues::ScatterGather(value) if value.list_address == 0x2000 && value.count == 8)
    );
    assert_eq!(linear.with_start(next_sg.start()), None);
    assert_eq!(sg.with_start(next_linear.start()), None);
    assert_eq!(linear.start(), Position::linear(3));
    assert_eq!(sg.start(), Position::scatter_gather(0, 0));
    Ok(())
}
