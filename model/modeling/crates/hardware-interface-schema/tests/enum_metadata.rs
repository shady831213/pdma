//! Shared enum metadata survives record graph composition.

use hardware_interface_schema::{
    Documentation, EnumDefinition, EnumVariantSpec, FieldDefinition, FieldSpec, RecordPlacement,
    RecordSpec, ResolvedMemberKind, ScalarKind,
};

const VARIANTS: [EnumVariantSpec; 2] = [
    EnumVariantSpec::new("idle", 0),
    EnumVariantSpec::new("active", 7).with_documentation(Documentation {
        short_description: Some("Active transfer"),
        description: Some("The transfer is running."),
    }),
];
const ENUM: EnumDefinition = EnumDefinition::new(&VARIANTS);
const FIELDS: [FieldSpec; 1] = [FieldSpec::place(
    0,
    "state",
    0,
    FieldDefinition::new(ScalarKind::U8).with_enum(ENUM),
)];
const CHILD: RecordSpec = RecordSpec::named("Child", 1, 1, &FIELDS, &[]);
const CHILDREN: [RecordPlacement; 1] = [RecordPlacement::new(0, "child", 0, &CHILD)];

#[test]
fn enum_variants_survive_nested_record_resolution() {
    // Given: a nested record whose scalar field carries a documented enum.
    let root = RecordSpec::named("Root", 1, 1, &[], &CHILDREN);
    // When: the root graph resolves the nested scalar.
    let resolved = root.resolve().expect("valid nested record");
    let definition = resolved
        .members()
        .iter()
        .find_map(|member| match member.kind() {
            ResolvedMemberKind::Field { definition, .. } => definition.enum_definition(),
            ResolvedMemberKind::Record { .. } => None,
        })
        .expect("enum metadata on nested scalar");
    // Then: declaration order, wire codes, and documentation remain available.
    let observed = definition
        .variants()
        .iter()
        .map(|variant| (variant.name(), variant.value(), variant.documentation()))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        [
            ("idle", 0, Documentation::EMPTY),
            (
                "active",
                7,
                Documentation {
                    short_description: Some("Active transfer"),
                    description: Some("The transfer is running."),
                }
            ),
        ]
    );
    assert_eq!(FIELDS[0].enum_definition(), Some(ENUM));
}
