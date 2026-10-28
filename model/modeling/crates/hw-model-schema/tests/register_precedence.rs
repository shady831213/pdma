//! Compound malformed-input and deterministic precedence coverage.

use hw_model_schema::Endian;
use hw_model_schema::register::{
    RegisterDefinition, RegisterDefinitionError, RegisterFieldDefinition, RegisterFieldSpec,
    ResetKind,
};
use hw_model_schema::register_map::{
    RegisterMapError, RegisterMapSpec, RegisterSpec, RepeatedRegisterBank,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldId {
    A,
    B,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterId {
    A,
    B,
}

const ONE: RegisterFieldDefinition = RegisterFieldDefinition::new(1);

#[test]
fn definition_validation_obeys_all_compound_phase_boundaries() {
    // Given: each row contains multiple defects and pins the first phase.
    const DUPLICATE: [RegisterFieldSpec<FieldId>; 2] = [
        RegisterFieldSpec::place(FieldId::A, "a", 0, ONE),
        RegisterFieldSpec::place(FieldId::A, "a", usize::MAX, ONE),
    ];
    const EXTENT_OVERLAP: [RegisterFieldSpec<FieldId>; 2] = [
        RegisterFieldSpec::place(FieldId::A, "a", 7, RegisterFieldDefinition::new(2)),
        RegisterFieldSpec::place(FieldId::B, "b", 7, ONE),
    ];
    const OVERLAP_COVERAGE: [RegisterFieldSpec<FieldId>; 2] = [
        RegisterFieldSpec::place(FieldId::A, "a", 0, RegisterFieldDefinition::new(2)),
        RegisterFieldSpec::place(FieldId::B, "b", 1, RegisterFieldDefinition::new(2)),
    ];
    let width = RegisterDefinition::<FieldId>::new("Width", 3, Endian::Little)
        .with_writable_mask(u64::MAX)
        .with_reset(ResetKind::Constant, u64::MAX);
    let mask = RegisterDefinition::<FieldId>::new("Mask", 1, Endian::Little)
        .with_writable_mask(0x100)
        .with_reset(ResetKind::Constant, 0x100);
    let duplicate = RegisterDefinition::new("Duplicate", 1, Endian::Little)
        .with_writable_mask(0x80)
        .with_fields(&DUPLICATE);
    let extent = RegisterDefinition::new("Extent", 1, Endian::Little)
        .with_writable_mask(0x80)
        .with_fields(&EXTENT_OVERLAP);
    let overlap = RegisterDefinition::new("Overlap", 1, Endian::Little)
        .with_writable_mask(0x80)
        .with_fields(&OVERLAP_COVERAGE);

    // When
    let results = [
        width.validate(),
        mask.validate(),
        duplicate.validate(),
        extent.validate(),
        overlap.validate(),
    ];

    // Then
    assert!(matches!(
        results[0],
        Err(RegisterDefinitionError::InvalidWidth { .. })
    ));
    assert!(matches!(
        results[1],
        Err(RegisterDefinitionError::WritableMaskOverflow { .. })
    ));
    assert_eq!(
        results[2],
        Err(RegisterDefinitionError::DuplicateFieldId {
            root: "Duplicate",
            path: "Duplicate.a".into(),
            declaration_index: 1,
            id: FieldId::A,
        })
    );
    assert!(matches!(
        results[3],
        Err(RegisterDefinitionError::FieldOutOfBounds { .. })
    ));
    assert!(matches!(
        results[4],
        Err(RegisterDefinitionError::FieldOverlap { .. })
    ));
}

#[test]
fn writable_coverage_reports_literal_masks_and_empty_fields_skip_it() {
    // Given
    const FIELDS: [RegisterFieldSpec<FieldId>; 1] = [RegisterFieldSpec::place(
        FieldId::A,
        "a",
        1,
        RegisterFieldDefinition::new(2),
    )];
    let covered = RegisterDefinition::new("Covered", 8, Endian::Little)
        .with_writable_mask(0x2)
        .with_fields(&FIELDS);
    let outside = RegisterDefinition::new("Outside", 8, Endian::Little)
        .with_writable_mask(0x9)
        .with_fields(&FIELDS);
    let whole =
        RegisterDefinition::<FieldId>::new("Whole", 8, Endian::Little).with_writable_mask(u64::MAX);

    // When
    let results = [covered.validate(), whole.validate(), outside.validate()];

    // Then
    assert_eq!(results[0], Ok(()));
    assert_eq!(
        results[2],
        Err(RegisterDefinitionError::WritableMaskOutsideFields {
            root: "Outside",
            path: "Outside".into(),
            declaration_index: 0,
            writable_mask: 0x9,
            declared_field_mask: 0x6,
            outside_mask: 0x9,
        })
    );
}

#[test]
fn field_arithmetic_phase_precedes_extent_and_reset_precedes_field_phases() {
    // Given
    const NAME_AND_ARITHMETIC: [RegisterFieldSpec<FieldId>; 2] = [
        RegisterFieldSpec::place(FieldId::A, "same", 7, RegisterFieldDefinition::new(2)),
        RegisterFieldSpec::place(FieldId::B, "same", usize::MAX, ONE),
    ];
    const EXTENT_AND_LATER_ARITHMETIC: [RegisterFieldSpec<FieldId>; 2] = [
        RegisterFieldSpec::place(FieldId::A, "extent", 7, RegisterFieldDefinition::new(2)),
        RegisterFieldSpec::place(FieldId::B, "arithmetic", usize::MAX, ONE),
    ];
    let duplicate_name =
        RegisterDefinition::new("Name", 1, Endian::Little).with_fields(&NAME_AND_ARITHMETIC);
    let arithmetic = RegisterDefinition::new("Arithmetic", 1, Endian::Little)
        .with_fields(&EXTENT_AND_LATER_ARITHMETIC);
    let reset = RegisterDefinition::new("Reset", 1, Endian::Little)
        .with_reset(ResetKind::Constant, 0x100)
        .with_fields(&NAME_AND_ARITHMETIC);

    // When
    let results = [
        duplicate_name.validate(),
        arithmetic.validate(),
        reset.validate(),
    ];

    // Then
    assert!(matches!(
        results[0],
        Err(RegisterDefinitionError::DuplicateFieldName {
            declaration_index: 1,
            ..
        })
    ));
    assert!(matches!(
        results[1],
        Err(RegisterDefinitionError::FieldBitArithmetic {
            declaration_index: 1,
            ..
        })
    ));
    assert!(matches!(
        results[2],
        Err(RegisterDefinitionError::ResetValueOverflow { .. })
    ));
}

#[test]
fn map_precedence_prefers_start_then_definition_then_later_layout() {
    // Given
    const FIELDS: [RegisterFieldSpec<FieldId>; 1] =
        [RegisterFieldSpec::place(FieldId::A, "a", 0, ONE)];
    const BAD: RegisterDefinition<FieldId> = RegisterDefinition::new("Bad", 1, Endian::Little)
        .with_writable_mask(0x2)
        .with_fields(&FIELDS);
    const BAD_PLACEMENT: [RegisterSpec<RegisterId, FieldId>; 1] =
        [RegisterSpec::place(RegisterId::A, "bad", 0, BAD)];
    const LATER: [RegisterSpec<RegisterId, FieldId>; 1] = [RegisterSpec::place(
        RegisterId::B,
        "later",
        0,
        RegisterDefinition::new("Later", 1, Endian::Little),
    )];
    const COLLIDING_BANK: [RepeatedRegisterBank<RegisterId, FieldId>; 1] =
        [RepeatedRegisterBank::new("collision", 0, 1, 1, &LATER)];
    const LATE_RANGE: [RepeatedRegisterBank<RegisterId, FieldId>; 1] =
        [RepeatedRegisterBank::new("late", usize::MAX, 2, 1, &LATER)];

    // When
    let duplicate_start =
        RegisterMapSpec::named("Start", 8, &BAD_PLACEMENT, &COLLIDING_BANK).resolve();
    let bad_definition =
        RegisterMapSpec::named("Definition", 8, &BAD_PLACEMENT, &LATE_RANGE).resolve();

    // Then
    assert!(matches!(
        duplicate_start,
        Err(RegisterMapError::DuplicateExpandedStart {
            declaration_index: 1,
            ..
        })
    ));
    assert!(matches!(
        bad_definition,
        Err(RegisterMapError::Definition(
            RegisterDefinitionError::WritableMaskOutsideFields {
                root: "Definition",
                declaration_index: 0,
                ..
            }
        ))
    ));
}

#[test]
fn repeated_layout_and_declaration_order_errors_carry_exact_paths() {
    // Given
    const DEFINITION: RegisterDefinition<FieldId> =
        RegisterDefinition::new("Value", 4, Endian::Little);
    const MEMBERS: [RegisterSpec<RegisterId, FieldId>; 1] =
        [RegisterSpec::place(RegisterId::A, "value", 2, DEFINITION)];
    const BANKS: [RepeatedRegisterBank<RegisterId, FieldId>; 1] =
        [RepeatedRegisterBank::new("bank", 4, 2, 4, &MEMBERS)];

    // When
    let result = RegisterMapSpec::named("Root", 16, &[], &BANKS).resolve();

    // Then: stride range failure precedes expanded overlap and uses the member declaration.
    assert_eq!(
        result,
        Err(RegisterMapError::BankStrideTooSmall {
            root: "Root",
            path: "Root.bank".into(),
            declaration_index: 0,
            stride: 4,
            required_extent: 6,
        })
    );
}
