//! Scatter/gather endpoint wire behavior.

use hardware_interface_schema::SchemaCodecError;
use pdma_model_contract::dma::{Command, Endpoint, EndpointMode, Response, Segment};

#[test]
fn independent_endpoints_and_progress_have_full_width_wire_storage() {
    // Given: the wire schemas used for one scatter/gather transfer.
    let command = Command::SPEC;
    let response = Response::SPEC;
    // When: their generated wire extents are inspected.
    let sizes = (command.size(), response.size());
    // Then: two tagged endpoints and 64-bit completion progress fit in the records.
    assert_eq!(sizes, (64, 24));
}

#[test]
fn endpoint_constructors_when_const_evaluated_select_matching_tags() -> Result<(), SchemaCodecError>
{
    // Given: independently chosen full-width linear and list parameters.
    const ENDPOINTS: [Endpoint; 2] = [
        Endpoint::linear(0x0102_0304_0506_0708, u64::MAX),
        Endpoint::scatter_gather(0x1112_1314_1516_1718, u64::MAX),
    ];
    // When: both endpoint representations cross the generated codec.
    for (endpoint, tag) in ENDPOINTS
        .into_iter()
        .zip([EndpointMode::Linear, EndpointMode::ScatterGather])
    {
        let bytes = endpoint.to_bytes()?;
        // Then: the tag and payload agree and preserve the full parameter width.
        assert_eq!(endpoint.mode, tag);
        assert_eq!(endpoint.payload.tag(), tag);
        assert_eq!(Endpoint::from_bytes(&bytes)?, endpoint);
        assert_eq!(bytes.len(), 24);
        assert_eq!(Endpoint::SPEC.alignment(), 8);
    }
    Ok(())
}

#[test]
fn endpoint_when_wire_mode_is_unknown_rejects_the_tag() {
    // Given: an endpoint containing the first unassigned mode.
    let mut bytes = [0; 24];
    bytes[0] = 2;
    // When: raw storage crosses the generated typed boundary.
    let result = Endpoint::from_bytes(&bytes);
    // Then: the mode's wire location and observed value identify the error.
    assert_eq!(
        result,
        Err(SchemaCodecError::UnknownTag {
            location: Endpoint::MODE_IDENTITY.at_byte(0),
            observed: 2,
        })
    );
}

#[test]
fn segment_when_encoded_preserves_full_width_little_endian_values() -> Result<(), SchemaCodecError>
{
    // Given: a descriptor with distinct address and length bytes.
    let segment = Segment {
        address: 0x0102_0304_0506_0708,
        length: 0x1112_1314_1516_1718,
    };
    // When: the generated descriptor codec encodes it.
    let bytes = segment.to_bytes()?;
    // Then: the 16-byte aligned record retains both complete values.
    assert_eq!(
        bytes,
        [
            8, 7, 6, 5, 4, 3, 2, 1, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11
        ]
    );
    assert_eq!(Segment::from_bytes(&bytes)?, segment);
    assert_eq!(Segment::SPEC.alignment(), 8);
    Ok(())
}
