//! Public module-path integration conformance.

use hardware_interface_schema::{codec, endian, record, scalar};

const FIELDS: [record::FieldSpec; 1] =
    [record::FieldSpec::new(1, "byte", 0, scalar::ScalarKind::U8)];

#[test]
fn public_modules_expose_representative_schema_items() {
    // Given
    let record = record::RecordSpec::new(1, 1, &FIELDS);
    let mut bytes = [0_u8; 1];

    // When
    let encoded = codec::write_scalar(&mut bytes, endian::Endian::Little, 0xa5_u8);
    let decoded = codec::read_scalar::<u8>(&bytes, endian::Endian::Little);
    let validation = record.validate();

    // Then
    assert_eq!(encoded, Ok(()));
    assert_eq!(decoded, Ok(0xa5));
    assert_eq!(validation, Ok(()));
}
