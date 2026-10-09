//! Typed subrange positions at the wire boundary.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{Endpoint, EndpointPayloadValues, Position};

#[test]
fn position_rejects_unknown_mode() {
    // Given: a position with an unassigned discriminator.
    let mut bytes = [0; Position::RECORD_SIZE];
    bytes[0] = 2;
    // When / Then: decoding cannot reinterpret it as an index or offset.
    assert!(matches!(
        Position::from_bytes(&bytes),
        Err(SchemaCodecError::UnknownTag { observed: 2, .. })
    ));
}

#[test]
fn positions_round_trip_full_width_and_reject_reserved_words()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both position variants with full-width fields.
    for position in [
        Position::linear(u64::MAX),
        Position::scatter_gather(u64::MAX - 1, u64::MAX),
    ] {
        // When: the generated codec roundtrips the value.
        let bytes = position.to_bytes()?;
        // Then: the variant and every payload bit survive.
        assert_eq!(Position::from_bytes(&bytes)?, position);
    }
    // Given: a Linear position with nonzero SG-only storage.
    let mut bytes = Position::linear(8).to_bytes()?;
    bytes[16] = 1;
    // When / Then: decoding rejects inactive storage.
    assert!(matches!(
        Position::from_bytes(&bytes),
        Err(SchemaCodecError::NonZeroReserved { observed: 1, .. })
    ));
    Ok(())
}

#[test]
fn subrange_preserves_geometry_and_rejects_cross_mode_positions()
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
    // When: software applies the two self-describing subrange positions.
    let next_linear = linear
        .with_start(Position::linear(40))
        .ok_or("linear subrange")?;
    let next_sg = sg
        .with_start(Position::scatter_gather(2, 5))
        .ok_or("SG subrange")?;
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
