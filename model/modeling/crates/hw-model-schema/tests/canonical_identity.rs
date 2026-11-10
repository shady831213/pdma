//! Source identities are stable across canonical graph collection.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{DefinitionBody, DefinitionId, Origin, SchemaSet},
    record_schema,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Leaf { endian: Big, fields: {
    bits: bits(u32) { fields: { flag: 0..1, } },
} } }
record_schema! { pub record Pair { endian: Little, fields: { first: Leaf, second: Leaf } } }
#[test]
fn repeated_named_references_share_their_normalized_anonymous_children() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    let root = graph.get(DefinitionId::of::<Pair>()).ok_or("pair")?;
    let targets: Vec<_> = root.references().iter().map(|r| r.target()).collect();
    assert_eq!(targets.len(), 2);
    for target in &targets {
        assert_eq!(target.id(), DefinitionId::of::<Leaf>());
        assert_eq!(target.origin(), Origin::Named);
        let bits = target.references().first().ok_or("bits")?.target();
        assert_eq!(bits.origin(), Origin::Inline);
        assert_eq!(bits.id().path(), DefinitionId::of::<Leaf>().path());
        assert!(matches!(bits.body(), DefinitionBody::Bits(_)));
        assert_eq!(
            graph.get(bits.id()).ok_or("canonical bits")?.name(),
            bits.name()
        );
    }
    assert_eq!(
        targets
            .first()
            .ok_or("first")?
            .references()
            .first()
            .ok_or("bits reference")?
            .target()
            .id(),
        targets
            .get(1)
            .ok_or("second")?
            .references()
            .first()
            .ok_or("bits reference")?
            .target()
            .id()
    );
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
#[test]
fn canonical_input_can_be_collected_again_without_lifting_new_types() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    let mut set = SchemaSet::new();
    set.add_definition(graph.get(DefinitionId::of::<Pair>()).ok_or("root")?.clone());
    let again = set.resolve()?;
    assert_eq!(
        graph
            .definitions()
            .map(|d| (d.id(), d.name(), d.origin()))
            .collect::<Vec<_>>(),
        again
            .definitions()
            .map(|d| (d.id(), d.name(), d.origin()))
            .collect::<Vec<_>>()
    );
    Ok(())
}
