//! Optional source rewrites preserve encoding and explicit containment.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::{DefinitionBody, DescribeSchema, FieldType, SchemaSet},
    record_schema,
    transform::{Inline, Outline},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Leaf { endian: Big, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { first: Leaf, second: Leaf } } }
#[test]
fn selected_reference_becomes_owned_without_changing_other_occurrences() -> TestResult {
    // Given repeated references to one reusable definition.
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    // When only the first definition-local occurrence is selected.
    let rewritten = Inline::new()
        .select(|site| site.path() == ["first"])
        .run(&graph)?
        .resolve()?;
    // Then its sibling remains an explicit reference and layout still analyzes.
    let root = rewritten
        .get(<Pair as DescribeSchema>::schema_definition().id())
        .ok_or("root")?;
    let DefinitionBody::Record(record) = root.body() else {
        return Err("record".into());
    };
    assert!(matches!(
        record.fields().first().ok_or("missing entry")?.ty(),
        FieldType::Record(_)
    ));
    assert!(matches!(
        record.fields().get(1).ok_or("missing entry")?.ty(),
        FieldType::Reference(_)
    ));
    assert_eq!(record.fields().get(1).ok_or("missing entry")?.offset(), 4);
    AnalyzedSchema::from_graph(&rewritten, &Bindings::new())?;
    Ok(())
}
#[test]
fn outline_restores_reusable_records_with_distinct_generated_identity() -> TestResult {
    // Given two owned copies of the same reusable source.
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    let inline = Inline::new().run(&graph)?.resolve()?;
    // When outlining all inline composites.
    let outlined = Outline::new().run(&inline)?.resolve()?;
    // Then every occurrence gets a unique source-derived identity.
    let root = outlined
        .get(<Pair as DescribeSchema>::schema_definition().id())
        .ok_or("root")?;
    let refs = root.references();
    assert_eq!(refs.len(), 2);
    assert_ne!(
        refs.first().ok_or("missing entry")?.target().id(),
        refs.get(1).ok_or("missing entry")?.target().id()
    );
    assert_eq!(outlined.roots().len(), 1);
    AnalyzedSchema::from_graph(&outlined, &Bindings::new())?;
    Ok(())
}
record_schema! { pub record Outer { endian: Little, fields: { one: Pair, two: Pair } } }
#[test]
fn rebuilt_repeated_edges_point_to_the_same_transformed_definition() -> TestResult {
    // Given two uses of a definition with its own repeated child references.
    let mut set = SchemaSet::new();
    set.add::<Outer>();
    // When selecting only Pair.first in its declaring definition's scope.
    let graph = Inline::new()
        .select(|site| {
            site.owner() == &hw_model_schema::ast::DefinitionId::of::<Pair>()
                && site.path() == ["first"]
        })
        .run(set)?
        .resolve()?;
    // Then both parent edges carry the transformed target rather than reconstructing its factory.
    let root = graph
        .get(hw_model_schema::ast::DefinitionId::of::<Outer>())
        .ok_or("root")?;
    assert_eq!(root.references().len(), 2);
    for reference in root.references() {
        let target = reference.target();
        let DefinitionBody::Record(record) = target.body() else {
            return Err("record".into());
        };
        assert!(matches!(
            record.fields().first().ok_or("missing entry")?.ty(),
            FieldType::Record(_)
        ));
        assert!(matches!(
            record.fields().get(1).ok_or("missing entry")?.ty(),
            FieldType::Reference(_)
        ));
    }
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
