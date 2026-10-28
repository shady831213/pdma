//! Documentation propagation through the public schema and resolution APIs.

use hw_model_schema::{
    BitsFieldDefinition, BitsFieldSpec, Documentation, Endian, FieldSpec, RecordPlacement,
    RecordSpec, RegisterDefinition, RegisterMapSpec, RegisterSpec, RepeatedRegisterBank,
    ScalarKind,
};

const TYPE_DOC: Documentation = Documentation {
    short_description: Some("Reusable type"),
    description: Some("Reusable type detail"),
};
const INSTANCE_DOC: Documentation = Documentation {
    short_description: Some("Specific placement"),
    description: None,
};
const EFFECTIVE_DOC: Documentation = Documentation {
    short_description: Some("Specific placement"),
    description: Some("Reusable type detail"),
};

#[test]
fn missing_parts_inherit_when_documentation_is_overridden() {
    // Given independently declared type and placement documentation.
    const OVERRIDE: Documentation = Documentation {
        short_description: Some(""),
        description: None,
    };
    // When fallback is evaluated at compile time.
    const EFFECTIVE: Documentation = OVERRIDE.with_fallback(TYPE_DOC);
    // Then explicit empty text overrides while missing text inherits.
    assert_eq!(EFFECTIVE.short_description, Some(""));
    assert_eq!(EFFECTIVE.description, TYPE_DOC.description);
    assert_eq!(Documentation::default(), Documentation::EMPTY);
}

#[test]
fn bit_placements_keep_independent_overrides_when_reusing_definitions() {
    // Given one documented reusable field shape.
    const DEFINITION: BitsFieldDefinition =
        BitsFieldDefinition::new(3).with_documentation(TYPE_DOC);
    // When two siblings reuse it with distinct documentation.
    let first =
        BitsFieldSpec::place(1_u16, "first", 0, DEFINITION).with_documentation(INSTANCE_DOC);
    let second = BitsFieldSpec::place(2_u16, "second", 3, DEFINITION);
    // Then documentation is local and geometry is unchanged.
    assert_eq!(first.documentation(), EFFECTIVE_DOC);
    assert_eq!(second.documentation(), TYPE_DOC);
    assert_eq!(first.range(), Some(0..3));
    assert_eq!(second.range(), Some(3..6));
}

#[test]
fn bit_descriptions_survive_when_embedded_in_record_fields() {
    // Given shared bit metadata embedded in a scalar field.
    static BITS: [BitsFieldSpec; 1] =
        [
            BitsFieldSpec::place(1, "flag", 0, BitsFieldDefinition::new(1))
                .with_documentation(INSTANCE_DOC),
        ];
    static FIELDS: [FieldSpec; 1] =
        [FieldSpec::new(1, "flags", 0, ScalarKind::U32).with_bits(&BITS)];
    // When a caller resolves the ordinary record graph.
    let resolved = RecordSpec::named("Root", 4, 4, &FIELDS, &[])
        .resolve()
        .expect("valid record");
    // Then existing field definitions expose the embedded bit documentation.
    let member = resolved.members().first().expect("field");
    let hw_model_schema::ResolvedMemberKind::Field { definition, .. } = member.kind() else {
        panic!("expected scalar field")
    };
    assert_eq!(
        definition.bits().first().expect("bit").documentation(),
        INSTANCE_DOC
    );
}

#[test]
fn ordered_collection_keeps_container_docs_separate_from_leaf_docs() {
    use hw_model_schema::{
        RegisterCollection, RegisterLayoutGeometry, RegisterMapPlacement,
        ResolvedRegisterMapPlacement, append_register_leaf, finish_ordered_register_map,
    };
    // Given a documented container and an independently documented register.
    let placement =
        RegisterMapPlacement::new(1_u16, "group", 0, 1, 0, RegisterLayoutGeometry::new(4, 4))
            .with_documentation(TYPE_DOC);
    let mut placements = vec![ResolvedRegisterMapPlacement::new(
        "Root.group".into(),
        placement,
    )];
    let mut registers = Vec::new();
    let mut declaration_index = 0;
    // When the ordinary collector appends a leaf and resolves the map.
    append_register_leaf(
        RegisterCollection {
            documentation: INSTANCE_DOC,
            id: 1,
            name: "value",
            path: "Root.group.value".into(),
            offset: 0,
            declaration_index: &mut declaration_index,
            repeated: None,
            placements: &mut placements,
            output: &mut registers,
        },
        RegisterDefinition::new("Value", 4, Endian::Little),
    );
    let resolved = finish_ordered_register_map("Root", 4, registers)
        .expect("valid map")
        .with_placements(placements);
    // Then container documentation stays queryable without becoming leaf fallback.
    let group = resolved.placements().first().expect("group");
    assert_eq!(
        (group.path(), group.documentation()),
        ("Root.group", TYPE_DOC)
    );
    assert_eq!(
        resolved
            .registers()
            .first()
            .expect("register")
            .documentation(),
        INSTANCE_DOC
    );
}

#[test]
fn nested_records_keep_instance_docs_when_resolved_without_ancestor_fallback() {
    // Given a documented child whose scalar has no documentation.
    static FIELDS: [FieldSpec; 1] = [FieldSpec::new(1, "value", 0, ScalarKind::U32)];
    static CHILD: RecordSpec =
        RecordSpec::named("Child", 4, 4, &FIELDS, &[]).with_documentation(TYPE_DOC);
    static PLACEMENTS: [RecordPlacement; 2] = [
        RecordPlacement::new(1, "override", 0, &CHILD).with_documentation(INSTANCE_DOC),
        RecordPlacement::new(2, "fallback", 4, &CHILD),
    ];
    let root =
        RecordSpec::named("Root", 8, 4, &[], &PLACEMENTS).with_documentation(Documentation {
            short_description: Some("Root only"),
            description: None,
        });
    // When the existing graph is resolved.
    let resolved = root.resolve().expect("valid record");
    // Then nested instances inherit only their referenced definition.
    let docs = resolved
        .members()
        .iter()
        .map(|member| (member.path(), member.documentation()))
        .collect::<Vec<_>>();
    assert_eq!(
        docs,
        vec![
            ("Root.override", EFFECTIVE_DOC),
            ("Root.override.value", Documentation::EMPTY),
            ("Root.fallback", TYPE_DOC),
            ("Root.fallback.value", Documentation::EMPTY),
        ]
    );
    assert_eq!(resolved.documentation(), root.documentation());
}

#[test]
fn register_instances_keep_overrides_when_banks_expand() {
    // Given a reused register definition and a documented bank.
    const DEFINITION: RegisterDefinition =
        RegisterDefinition::new("Value", 4, Endian::Little).with_documentation(TYPE_DOC);
    static REGISTERS: [RegisterSpec<u16>; 2] = [
        RegisterSpec::place(1, "first", 0, DEFINITION).with_documentation(INSTANCE_DOC),
        RegisterSpec::place(2, "second", 4, DEFINITION),
    ];
    static BANKS: [RepeatedRegisterBank<u16>; 1] =
        [
            RepeatedRegisterBank::new("bank", 0, 2, 8, &REGISTERS).with_documentation(
                Documentation {
                    short_description: Some("Bank only"),
                    description: None,
                },
            ),
        ];
    let map = RegisterMapSpec::named("Map", 16, &[], &BANKS).with_documentation(Documentation {
        short_description: Some("Map only"),
        description: None,
    });
    // When the existing register map expands the bank.
    let resolved = map.resolve().expect("valid map");
    // Then each register retains its own type fallback in every instance.
    let docs = resolved
        .registers()
        .iter()
        .map(|register| (register.offset(), register.documentation()))
        .collect::<Vec<_>>();
    assert_eq!(
        docs,
        vec![
            (0, EFFECTIVE_DOC),
            (8, EFFECTIVE_DOC),
            (4, TYPE_DOC),
            (12, TYPE_DOC),
        ]
    );
    assert_eq!(resolved.documentation(), map.documentation());
    assert_eq!(
        resolved.banks().first().expect("bank").documentation(),
        BANKS.first().expect("declared bank").documentation()
    );
}
