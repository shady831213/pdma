//! Value constraints and union shape boundary coverage.
use hw_model_schema::{
    analysis::{AnalysisError, AnalyzedSchema, validate::ShapeError},
    ast::*,
    *,
};

fn analyze(ty: FieldType) -> Result<AnalyzedSchema, AnalysisError> {
    AnalyzedSchema::record(
        "value",
        RecordDefinition::new(
            RecordSpec::new(1, 1, &[]),
            vec![Field::new("value", 0, 1, Endian::Little, ty)],
        ),
    )
}
#[test]
fn rejects_constant_overflow_with_typed_error() {
    let result = analyze(FieldType::Scalar {
        definition: FieldDefinition::new(ScalarKind::U8),
        constant: Some(Constant {
            symbol: "OVERFLOW",
            value: 256,
        }),
    });
    assert!(matches!(
        result,
        Err(AnalysisError::SourceShape {
            source: ShapeError::ValueOverflow,
            ..
        })
    ));
}
#[test]
fn rejects_duplicate_enum_codes() {
    static VARIANTS: [EnumVariantSpec; 2] =
        [EnumVariantSpec::new("a", 1), EnumVariantSpec::new("b", 1)];
    let result = analyze(FieldType::Enum(
        FieldDefinition::new(ScalarKind::U8).with_enum(EnumDefinition::new(&VARIANTS)),
    ));
    assert!(matches!(
        result,
        Err(AnalysisError::SourceShape {
            source: ShapeError::DuplicateVariant,
            ..
        })
    ));
}
#[test]
fn rejects_zero_width_bits() {
    static FIELDS: [BitsFieldSpec; 1] = [BitsFieldSpec::place(
        0,
        "zero",
        0,
        BitsFieldDefinition::new(0),
    )];
    let result = analyze(FieldType::Bits(BitsDefinition::new(
        ScalarKind::U8,
        &FIELDS,
    )));
    assert!(matches!(
        result,
        Err(AnalysisError::SourceShape {
            source: ShapeError::Bits(_),
            ..
        })
    ));
}
#[test]
fn rejects_bit_enum_code_overflow() {
    static VARIANTS: [EnumVariantSpec; 1] = [EnumVariantSpec::new("large", 2)];
    static FIELDS: [BitsFieldSpec; 1] = [BitsFieldSpec::place(
        0,
        "code",
        0,
        BitsFieldDefinition::new(1).with_enum(EnumDefinition::new(&VARIANTS)),
    )];
    let result = analyze(FieldType::Bits(BitsDefinition::new(
        ScalarKind::U8,
        &FIELDS,
    )));
    assert!(matches!(
        result,
        Err(AnalysisError::SourceShape {
            source: ShapeError::ValueOverflow,
            ..
        })
    ));
}
#[test]
fn rejects_tagged_inline_case_larger_than_union_storage() {
    let union = UnionDefinition::new(
        RecordSpec::new(1, 1, &[]),
        Some("tag"),
        vec![Case::inline(
            "large",
            Some(1),
            RecordDefinition::new(RecordSpec::new(8, 1, &[]), vec![]),
        )],
    );
    let result = analyze(FieldType::Union(Box::new(union)));
    assert!(matches!(
        result,
        Err(AnalysisError::SourceShape {
            source: ShapeError::Width {
                declared: 1,
                expected: 8
            },
            ..
        })
    ));
}
#[test]
fn accepts_union_case_with_storage_padding() {
    let union = UnionDefinition::new(
        RecordSpec::new(1, 1, &[]),
        None,
        vec![Case::inline(
            "empty",
            None,
            RecordDefinition::new(RecordSpec::new(0, 1, &[]), vec![]),
        )],
    );
    assert!(analyze(FieldType::Union(Box::new(union))).is_ok());
}

#[test]
fn rejects_register_bit_enum_code_overflow() {
    static VARIANTS: [EnumVariantSpec; 1] = [EnumVariantSpec::new("large", 2)];
    static FIELDS: [BitsFieldSpec; 1] = [BitsFieldSpec::place(
        0,
        "code",
        0,
        BitsFieldDefinition::new(1).with_enum(EnumDefinition::new(&VARIANTS)),
    )];
    let register = RegisterDefinition::new("register", 1, Endian::Little).with_fields(&FIELDS);
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register::<(), _>(register));
    let result = AnalyzedSchema::analyze(set, &analysis::Bindings::new());
    assert!(matches!(
        result,
        Err(AnalysisError::SourceShape {
            source: ShapeError::ValueOverflow,
            ..
        })
    ));
}
