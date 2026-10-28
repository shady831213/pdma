//! Scatter/gather endpoint wire behavior.

use hw_model_schema::SchemaCodecError;
use pdma_model_contract::dma::{Command, Event, Segment};

#[test]
fn independent_endpoints_and_progress_have_full_width_wire_storage() {
    // Given: the wire schemas used for one scatter/gather transfer.
    let command = Command::SPEC;
    let response = Event::SPEC;
    // When: their generated wire extents are inspected.
    let sizes = (command.size(), response.size());
    // Then: two tagged endpoints and 64-bit completion progress fit in the records.
    assert_eq!(sizes, (96, 24));
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
