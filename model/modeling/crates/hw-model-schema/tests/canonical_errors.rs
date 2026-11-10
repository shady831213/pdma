//! Canonical collection preserves auxiliary edges and rejects invalid graphs.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{Definition, DescribeSchema, FieldType, GraphError, Reference, ReferenceKind, SchemaSet},
    record_schema,
};
record_schema! { pub record Packet { endian: Little, fields: { value: u32 } } }
struct Recursive;
fn recursive() -> Definition {
    Definition::opaque::<Recursive>(hw_model_schema::RecordSpec::named(
        "Recursive",
        4,
        4,
        &[],
        &[],
    ))
    .with_references(vec![Reference::new(
        "loop",
        ReferenceKind::Field,
        recursive,
    )])
}
#[test]
fn canonical_collection_rejects_cycles() {
    // Given a recursive source graph.
    let mut set = SchemaSet::new();
    set.add_definition(recursive());
    // When canonical graph construction resolves its dependencies.
    let result = set.resolve();
    // Then the graph failure remains typed and chained.
    assert!(matches!(result, Err(GraphError::Cycle(_))));
}
#[test]
fn auxiliary_edges_are_preserved_without_inventing_containment() {
    // Given an opaque custom declaration with an auxiliary edge and no field owner.
    let definition = Definition::opaque::<Recursive>(hw_model_schema::RecordSpec::named(
        "Custom",
        4,
        4,
        &[],
        &[],
    ))
    .with_references(vec![Reference::new(
        "aux",
        ReferenceKind::Field,
        Packet::schema_definition,
    )]);
    let mut set = SchemaSet::new();
    set.add_definition(definition);
    // When the graph is constructed without requesting a containment rewrite.
    let result = set.resolve();
    // Then dependency provenance is retained without adding a physical field.
    let graph = result.unwrap();
    let definition = graph
        .get(hw_model_schema::ast::DefinitionId::of::<Recursive>())
        .unwrap();
    assert_eq!(definition.references().len(), 1);
    assert_eq!(
        definition
            .references()
            .first()
            .expect("auxiliary reference")
            .name(),
        "aux"
    );
}
#[test]
fn selector_binding_rejects_a_non_union_target() {
    // Given a manually supplied selector binding on an opaque target.
    struct Invalid;
    fn opaque() -> Definition {
        Definition::opaque::<Invalid>(hw_model_schema::RecordSpec::named("Opaque", 4, 4, &[], &[]))
    }
    let spec = hw_model_schema::RecordSpec::named("Owner", 4, 4, &[], &[]);
    let reference =
        Reference::new("value", ReferenceKind::Field, opaque).with_selector_binding("self.kind");
    let mut set = SchemaSet::new();
    set.add_definition(Definition::record::<Recursive>(
        spec,
        vec![hw_model_schema::ast::Field::new(
            "value",
            0,
            4,
            hw_model_schema::Endian::Little,
            FieldType::Reference(reference),
        )],
    ));
    // When normal analysis validates the source before any renderer.
    let result = AnalyzedSchema::analyze(set, &Bindings::new());
    // Then the malformed binding is a typed shape failure.
    assert!(matches!(
        result,
        Err(hw_model_schema::analysis::AnalysisError::SourceShape {
            source: hw_model_schema::analysis::validate::ShapeError::SelectorBinding,
            ..
        })
    ));
}
