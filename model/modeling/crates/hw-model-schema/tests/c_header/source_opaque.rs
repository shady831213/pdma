use super::support::{TestResult, compile_fragments, render_record, source};
use hw_model_schema::{
    Endian, FieldSpec, RecordPlacement, RecordSpec, ScalarKind,
    ast::{Definition, DescribeSchema, Field, FieldType, Reference, ReferenceKind},
};

struct Opaque;
const OPAQUE: RecordSpec = RecordSpec::named(
    "Opaque",
    2,
    2,
    &[FieldSpec::new(0, "value", 0, ScalarKind::U16)],
    &[],
);
impl DescribeSchema for Opaque {
    fn schema_definition() -> Definition {
        Definition::opaque::<Self>(OPAQUE)
    }
}
struct Container;
const CONTAINER: RecordSpec = RecordSpec::named(
    "Container",
    6,
    2,
    &[],
    &[RecordPlacement::new(0, "child", 4, &OPAQUE)],
);
impl DescribeSchema for Container {
    fn schema_definition() -> Definition {
        Definition::record::<Self>(
            CONTAINER,
            vec![Field::new(
                "child",
                4,
                2,
                Endian::Little,
                FieldType::Reference(Reference::new(
                    "child",
                    ReferenceKind::Field,
                    Opaque::schema_definition,
                )),
            )],
        )
    }
}

#[test]
fn opaque_reference_keeps_explicit_metadata_at_its_source_occurrence() -> TestResult {
    let actual = source::<Container>("container")?;
    let expected = render_record("container", "container", &CONTAINER)?;
    assert_eq!(actual.bytes, expected.bytes);
    compile_fragments(
        "source-opaque",
        &[&actual.bytes],
        br#"
_Static_assert(C_CONTAINER_C_CHILDQX2EVALUE_OFFSET == 4, "opaque child rebased");
int main(void) {
    uint8_t bytes[C_CONTAINER_SIZE] = {0};
    C_CONTAINER_STORE_C_CHILDQX2EVALUE(bytes, UINT16_C(0x1234));
    return bytes[4] != 0x34 || bytes[5] != 0x12;
}
"#,
    )
}
