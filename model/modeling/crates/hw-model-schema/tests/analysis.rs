//! Source-field validation is independent of physical storage lowering.
use hw_model_schema::ast::{Field, FieldType, RecordDefinition, analysis::Analysis};
use hw_model_schema::{Endian, FieldDefinition, RecordSpec, ScalarKind};

const fn field(name: &'static str, offset: usize) -> Field {
    Field::new(
        name,
        offset,
        4,
        Endian::Little,
        FieldType::Scalar {
            definition: FieldDefinition::new(ScalarKind::U32),
            constant: None,
        },
    )
}

#[test]
fn manual_source_fields_are_checked_independently_of_storage_lowering() {
    let storage = RecordSpec::new(8, 4, &[]);
    let valid = RecordDefinition::new(storage, vec![field("left", 0), field("right", 4)]);
    assert!(Analysis::record("valid", valid).is_ok());
    let outside = RecordDefinition::new(storage, vec![field("outside", 6)]);
    assert!(Analysis::record("outside", outside).is_err());
    let overlap = RecordDefinition::new(storage, vec![field("left", 0), field("right", 2)]);
    assert!(Analysis::record("overlap", overlap).is_err());
}
