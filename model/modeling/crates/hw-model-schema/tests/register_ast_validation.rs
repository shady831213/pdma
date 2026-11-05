//! Register AST validation preserves runtime field geometry and diagnostic order.
use hw_model_schema::{
    Endian, RegisterDefinition, RegisterFieldDefinition, RegisterFieldSpec, ResetKind,
    analysis::{AnalysisError, AnalyzedSchema, Bindings},
    ast::{Definition, RegisterInfo, SchemaSet},
};

fn analyze(spec: RegisterDefinition<usize>) -> Result<AnalyzedSchema, AnalysisError> {
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register::<(), _>(spec));
    AnalyzedSchema::analyze(set, &Bindings::new())
}

fn rejects_like_runtime(spec: RegisterDefinition<usize>) {
    // Given: the public runtime validator rejects a malformed register definition.
    let expected = spec.validate();
    assert!(expected.is_err(), "malformed runtime definition");
    // When: the same definition enters the public AST and analysis boundaries.
    let projected = RegisterInfo::from_spec(spec).validate_shape();
    let analyzed = analyze(spec);
    // Then: projection retains the runtime diagnostic and analysis rejects it.
    assert_eq!(projected, expected);
    assert!(matches!(analyzed, Err(AnalysisError::Register { .. })));
}

#[test]
fn rejects_out_of_bounds_fields() {
    const FIELDS: [RegisterFieldSpec<usize>; 1] = [RegisterFieldSpec::place(
        0,
        "outside",
        63,
        RegisterFieldDefinition::new(8),
    )];
    rejects_like_runtime(RegisterDefinition::new("Bounds", 1, Endian::Little).with_fields(&FIELDS));
}

#[test]
fn rejects_zero_width_fields() {
    const FIELDS: [RegisterFieldSpec<usize>; 1] = [RegisterFieldSpec::place(
        0,
        "empty",
        0,
        RegisterFieldDefinition::new(0),
    )];
    rejects_like_runtime(RegisterDefinition::new("Zero", 1, Endian::Little).with_fields(&FIELDS));
}

#[test]
fn rejects_field_arithmetic_overflow() {
    const FIELDS: [RegisterFieldSpec<usize>; 1] = [RegisterFieldSpec::place(
        0,
        "overflow",
        usize::MAX,
        RegisterFieldDefinition::new(2),
    )];
    rejects_like_runtime(
        RegisterDefinition::new("Arithmetic", 1, Endian::Little).with_fields(&FIELDS),
    );
}

#[test]
fn rejects_duplicate_names_before_invalid_ranges() {
    const FIELDS: [RegisterFieldSpec<usize>; 2] = [
        RegisterFieldSpec::place(0, "same", 0, RegisterFieldDefinition::new(1)),
        RegisterFieldSpec::place(1, "same", usize::MAX, RegisterFieldDefinition::new(1)),
    ];
    rejects_like_runtime(RegisterDefinition::new("Names", 1, Endian::Little).with_fields(&FIELDS));
}

#[test]
fn rejects_overlapping_fields_in_explicit_declaration_order() {
    const FIELDS: [RegisterFieldSpec<usize>; 2] = [
        RegisterFieldSpec::place(0, "later", 0, RegisterFieldDefinition::new(2))
            .with_declaration_index(7),
        RegisterFieldSpec::place(1, "earlier", 1, RegisterFieldDefinition::new(2))
            .with_declaration_index(3),
    ];
    rejects_like_runtime(
        RegisterDefinition::new("Overlap", 1, Endian::Little).with_fields(&FIELDS),
    );
}

#[test]
fn rejects_writable_bits_outside_fields() {
    const FIELDS: [RegisterFieldSpec<usize>; 1] = [RegisterFieldSpec::place(
        0,
        "value",
        1,
        RegisterFieldDefinition::new(2),
    )];
    rejects_like_runtime(
        RegisterDefinition::new("Mask", 1, Endian::Little)
            .with_writable_mask(0x81)
            .with_fields(&FIELDS),
    );
}

#[test]
fn rejects_reset_overflow_before_field_errors() {
    const FIELDS: [RegisterFieldSpec<usize>; 1] = [RegisterFieldSpec::place(
        0,
        "empty",
        0,
        RegisterFieldDefinition::new(0),
    )];
    rejects_like_runtime(
        RegisterDefinition::new("Reset", 1, Endian::Little)
            .with_reset(ResetKind::Constant, 0x100)
            .with_fields(&FIELDS),
    );
}

#[test]
fn accepts_masked_fields_with_gaps_and_whole_registers() {
    // Given: writable bits are a subset of a sparse field layout; reset fits storage.
    const FIELDS: [RegisterFieldSpec<usize>; 2] = [
        RegisterFieldSpec::place(0, "low", 1, RegisterFieldDefinition::new(2)),
        RegisterFieldSpec::place(1, "high", 7, RegisterFieldDefinition::new(1)),
    ];
    let masked = RegisterDefinition::new("Masked", 1, Endian::Little)
        .with_fields(&FIELDS)
        .with_writable_mask(0x82)
        .with_reset(ResetKind::Constant, 0xff);
    let whole = RegisterDefinition::<usize>::new("Whole", 8, Endian::Big)
        .with_writable_mask(u64::MAX)
        .with_reset(ResetKind::Constant, u64::MAX);
    // When: runtime and AST validate each supported shape.
    for spec in [masked, whole] {
        let runtime = spec.validate();
        let projected = RegisterInfo::from_spec(spec).validate_shape();
        let analyzed = analyze(spec);
        // Then: field gaps, partial writable masks and fieldless registers remain legal.
        assert_eq!(runtime, Ok(()));
        assert_eq!(projected, Ok(()));
        assert!(analyzed.is_ok());
    }
}
