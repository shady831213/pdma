//! Mixed-invalid record validation precedence tests.

use hw_model_schema::{FieldSpec, RecordSpec, RecordValidationError, ScalarKind};

const fn field(id: u16, name: &'static str, offset: usize, scalar: ScalarKind) -> FieldSpec {
    FieldSpec::new(id, name, offset, scalar)
}

#[test]
fn record_validation_prioritizes_zero_alignment_over_extent() {
    // Given
    const FIELDS: [FieldSpec; 1] = [field(71, "tail", 6, ScalarKind::U32)];

    // When
    let result = RecordSpec::new(8, 0, &FIELDS).validate();

    // Then
    assert_eq!(result, Err(RecordValidationError::AlignmentZero));
}

#[test]
fn record_validation_prioritizes_non_power_of_two_alignment_over_extent() {
    // Given
    const FIELDS: [FieldSpec; 1] = [field(72, "tail", 6, ScalarKind::U32)];

    // When
    let result = RecordSpec::new(8, 3, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::AlignmentNotPowerOfTwo { alignment: 3 })
    );
}

#[test]
fn record_validation_prioritizes_alignment_over_overlap() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(73, "first", 2, ScalarKind::U32),
        field(74, "second", 4, ScalarKind::U16),
    ];

    // When
    let result = RecordSpec::new(8, 0, &FIELDS).validate();

    // Then
    assert_eq!(result, Err(RecordValidationError::AlignmentZero));
}

#[test]
fn record_validation_prioritizes_extent_over_overlap() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        field(75, "first", 6, ScalarKind::U32),
        field(76, "second", 6, ScalarKind::U32),
    ];

    // When
    let result = RecordSpec::new(8, 4, &FIELDS).validate();

    // Then
    assert_eq!(
        result,
        Err(RecordValidationError::FieldOutOfBounds {
            field_id: 75,
            offset: 6,
            width: 4,
            record_size: 8,
        })
    );
}
