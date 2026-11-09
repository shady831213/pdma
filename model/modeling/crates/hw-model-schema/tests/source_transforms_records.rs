//! Source selection preserves tags, bit metadata, identities and failures.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::{Definition, DefinitionBody, DefinitionId, FieldType, SchemaSet},
    record_schema,
    transform::{Inline, Outline, TransformError},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Packet { endian: Big, fields: {
    flags: bits(u32) { fields: { active: 0..1, } },
    kind: enum(u32) { variants: { Empty = 0, Data = 7 } },
    payload: tagged_union { tag: self.kind, cases: { Empty {}, Data { number: u32 } } },
} } }
#[test]
fn selective_outline_lifts_bits_and_one_case_without_lifting_union() -> TestResult {
    // Given source-owned bits and tagged inline union alternatives.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    // When only bits and the Data case are selected.
    let graph = Outline::new()
        .select(|site| site.path() == ["flags"] || site.path() == ["payload", "Data"])
        .run(set)?
        .resolve()?;
    // Then the union stays inline with its selector and wire discriminants.
    let root = graph.get(DefinitionId::of::<Packet>()).ok_or("root")?;
    let DefinitionBody::Record(record) = root.body() else {
        return Err("record".into());
    };
    assert!(matches!(
        record.fields().first().ok_or("missing entry")?.ty(),
        FieldType::Reference(_)
    ));
    assert_eq!(
        record.fields().first().ok_or("missing entry")?.endian(),
        hw_model_schema::Endian::Big
    );
    assert_eq!(record.fields().get(2).ok_or("missing entry")?.offset(), 8);
    let FieldType::Union(union) = record.fields().get(2).ok_or("missing entry")?.ty() else {
        return Err("inline union".into());
    };
    assert_eq!(union.selector(), Some("self.kind"));
    assert_eq!(
        union.cases().get(1).ok_or("missing entry")?.discriminant(),
        Some(7)
    );
    assert!(matches!(
        union.cases().first().ok_or("missing entry")?.body(),
        hw_model_schema::ast::CaseBody::Inline(_)
    ));
    assert!(
        matches!(union.cases().get(1).ok_or("missing entry")?.body(), hw_model_schema::ast::CaseBody::Reference(reference) if reference.discriminant() == Some(7))
    );
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
#[test]
fn generated_names_do_not_define_identity_and_collisions_are_typed() -> TestResult {
    // Given the same source under two application naming policies.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let graph = set.resolve()?;
    // When outlining bits with independently chosen names.
    let first = Outline::new()
        .select(|site| site.path() == ["flags"])
        .name_with(|_| "One".into())
        .run(&graph)?
        .resolve()?;
    let second = Outline::new()
        .select(|site| site.path() == ["flags"])
        .name_with(|_| "Two".into())
        .run(&graph)?
        .resolve()?;
    let first = first
        .definitions()
        .find(|d| d.name() == "One")
        .ok_or("first")?;
    let second = second
        .definitions()
        .find(|d| d.name() == "Two")
        .ok_or("second")?;
    // Then identity is derived from source structure, while colliding names are rejected.
    assert_eq!(first.id(), second.id());
    assert_eq!(first.id().path(), DefinitionId::of::<Packet>().path());
    assert!(!first.id().structural_path().is_empty());
    assert!(
        matches!(Outline::new().name_with(|_| "Same".into()).run(&graph), Err(TransformError::NameCollision(name)) if name == "Same")
    );
    Ok(())
}
#[test]
fn complete_outline_then_inline_restores_union_shape_and_stable_order() -> TestResult {
    // Given the same selected source across independent graph collections.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let graph = set.resolve()?;
    // When fully outlining twice, then expanding one result.
    let first = Outline::new().run(&graph)?.resolve()?;
    let second = Outline::new().run(&graph)?.resolve()?;
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
    let restored = Inline::new().run(first)?.resolve()?;
    // Then union containment and its original selector survive the roundtrip.
    let root = restored.get(DefinitionId::of::<Packet>()).ok_or("root")?;
    let DefinitionBody::Record(record) = root.body() else {
        return Err("record".into());
    };
    let FieldType::Union(union) = record.fields().get(2).ok_or("missing entry")?.ty() else {
        return Err("union".into());
    };
    assert_eq!(union.selector(), Some("self.kind"));
    assert!(matches!(
        union.cases().get(1).ok_or("missing entry")?.body(),
        hw_model_schema::ast::CaseBody::Inline(_)
    ));
    AnalyzedSchema::from_graph(&restored, &Bindings::new())?;
    Ok(())
}
#[test]
fn outlined_selector_is_bound_at_use_site_not_in_reusable_payload() -> TestResult {
    // Given a union selector referring to its containing record's kind field.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    // When that union is lifted into a reusable payload definition.
    let graph = Outline::new()
        .select(|site| site.path() == ["payload"])
        .run(set)?
        .resolve()?;
    // Then the lexical binding remains on the field edge, while the independent payload has none.
    let root = graph.get(DefinitionId::of::<Packet>()).ok_or("root")?;
    let reference = root
        .references()
        .into_iter()
        .find(|r| r.name() == "payload")
        .ok_or("payload")?;
    assert_eq!(reference.selector_binding(), Some("self.kind"));
    let target = reference.target();
    let DefinitionBody::Union(union) = target.body() else {
        return Err("union".into());
    };
    assert_eq!(union.selector(), None);
    assert_eq!(
        union.cases().get(1).ok_or("missing entry")?.discriminant(),
        Some(7)
    );
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
#[test]
fn outline_preserves_inline_definition_and_use_site_documentation() -> TestResult {
    // Given independently documented inline definition and containing field.
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
    let root = Definition::record::<DocumentedOwner>(
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
    // When the inline declaration becomes reusable.
    let graph = Outline::new().run(set)?.resolve()?;
    // Then neither definition documentation nor placement documentation replaces the other.
    let root = graph
        .get(DefinitionId::of::<DocumentedOwner>())
        .ok_or("root")?;
    let DefinitionBody::Record(record) = root.body() else {
        return Err("record".into());
    };
    assert_eq!(
        record
            .fields()
            .first()
            .ok_or("missing entry")?
            .documentation(),
        field_doc
    );
    assert_eq!(
        root.references()
            .first()
            .ok_or("missing entry")?
            .target()
            .documentation(),
        definition_doc
    );
    Ok(())
}

struct DocumentedOwner;
