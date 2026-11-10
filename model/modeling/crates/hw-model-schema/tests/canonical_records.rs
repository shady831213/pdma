//! Canonical definitions retain original declaration provenance and selector scope.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{Definition, DefinitionBody, DefinitionId, FieldType, Origin, SchemaSet},
    record_schema,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Packet { endian: Big, fields: {
    flags: bits(u32) { fields: { active: 0..1, mode: 1..3 => enum { variants: { Off=0, On=1 } }, } },
    kind: enum(u32) { variants: { Empty=0, Data=7 } },
    payload: tagged_union { tag: self.kind, cases: { Empty {}, Data { number:u32 } } },
} } }
#[test]
fn enums_in_records_and_bits_are_defined_types_not_builtin_scalars() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let graph = set.resolve()?;
    let packet = graph.get(DefinitionId::of::<Packet>()).ok_or("packet")?;
    assert_eq!(packet.origin(), Origin::Named);
    for reference in packet.references() {
        assert_eq!(reference.target().origin(), Origin::Inline);
    }
    let flags = packet
        .references()
        .into_iter()
        .find(|r| r.name() == "flags")
        .ok_or("flags")?
        .target();
    assert_eq!(flags.references().len(), 1);
    let mode = flags.references().first().ok_or("mode")?.target();
    assert!(matches!(mode.body(), DefinitionBody::Enumeration(_)));
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
#[test]
fn selector_belongs_to_use_site_and_case_discriminants_survive_normalization() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let graph = set.resolve()?;
    let root = graph.get(DefinitionId::of::<Packet>()).ok_or("root")?;
    let payload = root
        .references()
        .into_iter()
        .find(|r| r.name() == "payload")
        .ok_or("payload")?;
    assert_eq!(payload.selector_binding(), Some("self.kind"));
    let target = payload.target();
    let DefinitionBody::Union(union) = target.body() else {
        return Err("union".into());
    };
    assert_eq!(union.selector(), None);
    assert_eq!(union.cases().get(1).ok_or("case")?.discriminant(), Some(7));
    assert!(
        target
            .references()
            .iter()
            .all(|r| r.target().origin() == Origin::Inline)
    );
    Ok(())
}
#[test]
fn source_derived_identity_and_order_are_stable_across_collections() -> TestResult {
    let mut first = SchemaSet::new();
    first.add::<Packet>();
    let mut second = SchemaSet::new();
    second.add::<Packet>();
    let first = first.resolve()?;
    let second = second.resolve()?;
    assert_eq!(
        first
            .definitions()
            .map(|d| (d.id(), d.name()))
            .collect::<Vec<_>>(),
        second
            .definitions()
            .map(|d| (d.id(), d.name()))
            .collect::<Vec<_>>()
    );
    let ids: std::collections::HashSet<_> = first.definitions().map(Definition::id).collect();
    assert_eq!(ids.len(), first.definitions().count());
    Ok(())
}
#[test]
fn manually_constructed_inline_definitions_share_the_same_normalization() -> TestResult {
    struct Owner;
    let definition_doc = hw_model_schema::Documentation {
        short_description: Some("definition"),
        description: None,
    };
    let field_doc = hw_model_schema::Documentation {
        short_description: Some("field"),
        description: None,
    };
    let inner = hw_model_schema::ast::RecordDefinition::new(
        hw_model_schema::RecordSpec::named("Inner", 4, 4, &[], &[])
            .with_documentation(definition_doc),
        vec![],
    );
    let root = Definition::record::<Owner>(
        hw_model_schema::RecordSpec::named("Outer", 4, 4, &[], &[]),
        vec![
            hw_model_schema::ast::Field::new(
                "child",
                0,
                4,
                hw_model_schema::Endian::Little,
                FieldType::Record(Box::new(inner)),
            )
            .with_documentation(field_doc),
        ],
    );
    let mut set = SchemaSet::new();
    set.add_definition(root);
    let graph = set.resolve()?;
    let root = graph.get(DefinitionId::of::<Owner>()).ok_or("root")?;
    let DefinitionBody::Record(record) = root.body() else {
        return Err("record".into());
    };
    assert_eq!(
        record.fields().first().ok_or("field")?.documentation(),
        field_doc
    );
    let child = root.references().first().ok_or("child")?.target();
    assert_eq!(child.documentation(), definition_doc);
    assert_eq!(child.origin(), Origin::Inline);
    assert!(!child.id().structural_path().is_empty());
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
