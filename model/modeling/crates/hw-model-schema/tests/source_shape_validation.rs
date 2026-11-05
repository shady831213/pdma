//! Public AST shape validation before generation.
use hw_model_schema::{analysis::AnalyzedSchema, ast::*, *};

fn analyze(ty: FieldType, width: usize) -> Result<AnalyzedSchema, analysis::AnalysisError> {
    AnalyzedSchema::record(
        "source",
        RecordDefinition::new(
            RecordSpec::new(width, 1, &[]),
            vec![Field::new("value", 0, width, Endian::Little, ty)],
        ),
    )
}
const fn scalar(definition: FieldDefinition) -> FieldType {
    FieldType::Scalar {
        definition,
        constant: None,
    }
}
fn opaque() -> Definition {
    Definition::opaque::<u64>(RecordSpec::new(8, 1, &[]))
}
#[test]
fn rejects_scalar_extent_mismatch() {
    assert!(analyze(scalar(FieldDefinition::new(ScalarKind::U64)), 1).is_err());
}
#[test]
fn rejects_scalar_definition_width_mismatch() {
    assert!(
        analyze(
            scalar(FieldDefinition::new(ScalarKind::U64).with_width(1)),
            1
        )
        .is_err()
    );
}
#[test]
fn rejects_scalar_endian_mismatch() {
    assert!(
        analyze(
            scalar(FieldDefinition::new(ScalarKind::U8).with_endian(Endian::Big)),
            1
        )
        .is_err()
    );
}
#[test]
fn rejects_reference_extent_mismatch() {
    assert!(
        analyze(
            FieldType::Reference(Reference::new("child", ReferenceKind::Field, opaque)),
            1
        )
        .is_err()
    );
}
#[test]
fn rejects_bits_extent_mismatch() {
    assert!(
        analyze(
            FieldType::Bits(BitsDefinition::new(ScalarKind::U64, &[])),
            1
        )
        .is_err()
    );
}
#[test]
fn rejects_overlapping_bits() {
    static FIELDS: [BitsFieldSpec; 2] = [
        BitsFieldSpec::place(0, "a", 0, BitsFieldDefinition::new(4)),
        BitsFieldSpec::place(1, "b", 3, BitsFieldDefinition::new(4)),
    ];
    assert!(
        analyze(
            FieldType::Bits(BitsDefinition::new(ScalarKind::U8, &FIELDS)),
            1
        )
        .is_err()
    );
}
#[test]
fn rejects_out_of_bounds_bits() {
    static FIELDS: [BitsFieldSpec; 1] =
        [BitsFieldSpec::place(0, "a", 7, BitsFieldDefinition::new(2))];
    assert!(
        analyze(
            FieldType::Bits(BitsDefinition::new(ScalarKind::U8, &FIELDS)),
            1
        )
        .is_err()
    );
}
#[test]
fn rejects_oversized_union_case() {
    let union = UnionDefinition::new(
        RecordSpec::new(1, 1, &[]),
        None,
        vec![Case::external(Reference::new(
            "child",
            ReferenceKind::Variant,
            opaque,
        ))],
    );
    assert!(analyze(FieldType::Union(Box::new(union)), 1).is_err());
}
#[test]
fn rejects_enum_code_overflow() {
    static VARIANTS: [EnumVariantSpec; 1] = [EnumVariantSpec::new("large", 256)];
    let definition = FieldDefinition::new(ScalarKind::U8).with_enum(EnumDefinition::new(&VARIANTS));
    assert!(analyze(FieldType::Enum(definition), 1).is_err());
}
#[test]
fn accepts_packed_fields_and_opaque_padding() {
    let record = RecordDefinition::new(
        RecordSpec::new(12, 1, &[]),
        vec![Field::new(
            "packed",
            1,
            8,
            Endian::Little,
            scalar(FieldDefinition::new(ScalarKind::U64)),
        )],
    );
    assert!(AnalyzedSchema::record("packed", record).is_ok());
}
