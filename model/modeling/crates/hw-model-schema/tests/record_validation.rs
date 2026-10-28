//! Fixed-record schema validation tests.

use hw_model_schema::{Endian, FieldSpec, RecordSpec, RecordValidationError, ScalarKind};

const fn field(id: u16, name: &'static str, offset: usize, scalar: ScalarKind) -> FieldSpec {
    FieldSpec::new(id, name, offset, scalar)
}

#[test]
fn record_validation_permits_gaps() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(1, "first", 0, ScalarKind::U8),
        field(2, "second", 4, ScalarKind::U32).with_endian(Endian::Big),
    ];
    let record = RecordSpec::new(8, 8, &FIELDS);

    // When
    let result = record.validate();

    // Then
    assert_eq!(result, Ok(()));
    assert_eq!(record.fields(), &FIELDS);
}

#[test]
fn record_validation_rejects_duplicate_field_id() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(7, "first", 0, ScalarKind::U8),
        field(7, "second", 1, ScalarKind::U8),
    ];

    // When
    let result = RecordSpec::new(2, 1, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::DuplicateFieldId { id: 7 })
    );
}

#[test]
fn record_validation_rejects_duplicate_field_name() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(1, "same", 0, ScalarKind::U8),
        field(2, "same", 1, ScalarKind::U8),
    ];

    // When
    let result = RecordSpec::new(2, 1, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::DuplicateFieldName { name: "same" })
    );
}

#[test]
fn record_validation_rejects_zero_alignment() {
    // Given
    const FIELDS: [FieldSpec; 0] = [];

    // When
    let result = RecordSpec::new(0, 0, &FIELDS).validate();

    // Then
    assert_eq!(result, Err(RecordValidationError::AlignmentZero));
}

#[test]
fn record_validation_rejects_non_power_of_two_alignment() {
    // Given
    const FIELDS: [FieldSpec; 0] = [];

    // When
    let result = RecordSpec::new(0, 3, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::AlignmentNotPowerOfTwo { alignment: 3 })
    );
}

#[test]
fn record_validation_rejects_scalar_width_mismatch() {
    // Given
    const FIELDS: [FieldSpec; 1] = [field(9, "word", 0, ScalarKind::U32).with_width(2)];

    // When
    let result = RecordSpec::new(4, 4, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::ScalarWidthMismatch {
            field_id: 9,
            scalar: ScalarKind::U32,
            width: 2,
        })
    );
}

#[test]
fn record_validation_rejects_overlap_in_declaration_order() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(11, "first", 2, ScalarKind::U32),
        field(12, "second", 4, ScalarKind::U16),
    ];

    // When
    let result = RecordSpec::new(8, 4, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::FieldOverlap {
            first_id: 11,
            second_id: 12,
        })
    );
}

#[test]
fn record_validation_rejects_out_of_bounds_extent() {
    // Given
    const FIELDS: [FieldSpec; 1] = [field(21, "tail", 6, ScalarKind::U32)];

    // When
    let result = RecordSpec::new(8, 4, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::FieldOutOfBounds {
            field_id: 21,
            offset: 6,
            width: 4,
            record_size: 8,
        })
    );
}

#[test]
fn record_validation_prioritizes_duplicate_id_over_zero_alignment() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(31, "first", 0, ScalarKind::U8),
        field(31, "second", 1, ScalarKind::U8),
    ];

    // When
    let result = RecordSpec::new(2, 0, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::DuplicateFieldId { id: 31 })
    );
}

#[test]
fn record_validation_prioritizes_duplicate_id_over_non_power_of_two_alignment() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(32, "first", 0, ScalarKind::U8),
        field(32, "second", 1, ScalarKind::U8),
    ];

    // When
    let result = RecordSpec::new(2, 3, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::DuplicateFieldId { id: 32 })
    );
}

#[test]
fn record_validation_prioritizes_duplicate_id_over_duplicate_name() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(33, "same", 0, ScalarKind::U8),
        field(33, "same", 1, ScalarKind::U8),
    ];

    // When
    let result = RecordSpec::new(2, 1, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::DuplicateFieldId { id: 33 })
    );
}

#[test]
fn record_validation_prioritizes_duplicate_name_over_field_shape_extent_and_overlap() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(41, "repeated", 6, ScalarKind::U32).with_width(2),
        field(42, "repeated", 6, ScalarKind::U32),
    ];

    // When
    let result = RecordSpec::new(7, 4, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::DuplicateFieldName { name: "repeated" })
    );
}

#[test]
fn record_validation_prioritizes_scalar_width_over_extent_and_overlap() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(51, "narrow", 6, ScalarKind::U32).with_width(2),
        field(52, "overlap", 6, ScalarKind::U32),
    ];

    // When
    let result = RecordSpec::new(7, 4, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::ScalarWidthMismatch {
            field_id: 51,
            scalar: ScalarKind::U32,
            width: 2,
        })
    );
}

#[test]
fn record_validation_prioritizes_scalar_width_over_alignment() {
    // Given
    const FIELDS: [FieldSpec; 1] = [field(61, "word", 0, ScalarKind::U32).with_width(2)];

    // When
    let result = RecordSpec::new(4, 0, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::ScalarWidthMismatch {
            field_id: 61,
            scalar: ScalarKind::U32,
            width: 2,
        })
    );
}
