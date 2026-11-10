//! Typed pipeline composition and occurrence-bound source traversal.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{SchemaSet, record::Node},
    record_schema,
};
record_schema! { pub record Header { endian: Little, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { left: Header, right: Header } } }
fn schema() -> Result<AnalyzedSchema, hw_model_schema::analysis::AnalysisError> {
    let mut set = SchemaSet::new();
    set.add_record::<Pair>();
    AnalyzedSchema::analyze(set, &Bindings::new())
}
#[test]
fn shared_referents_preserve_distinct_occurrence_ancestry() {
    let schema = schema().unwrap();
    let root = hw_model_schema::traversal::record::definitions(&schema)
        .find(|v| v.context().unit().name == "Pair")
        .unwrap();
    let mut paths = Vec::new();
    let mut definitions = Vec::new();
    for field in root
        .context()
        .children()
        .filter(|v| matches!(v.node(), Node::Field(_)))
    {
        assert!(
            field
                .context()
                .children()
                .all(|v| !matches!(v.node(), Node::Record(_)))
        );
        assert!(field.context().referent().is_none());
        let reference = field
            .context()
            .children()
            .find(|v| matches!(v.node(), Node::Reference(_)))
            .unwrap();
        let referent = reference.context().referent().unwrap();
        assert_eq!(referent.context().unit().name, "Pair");
        let Node::Record(definition) = referent.node() else {
            panic!("record expected")
        };
        definitions.push(core::ptr::from_ref(definition));
        for child in referent
            .context()
            .children()
            .filter(|v| matches!(v.node(), Node::Field(_)))
        {
            let path = child
                .context()
                .ancestors()
                .iter()
                .copied()
                .chain([child.node()])
                .filter_map(|node| match node {
                    Node::Field(f) => Some(f.name()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(".");
            paths.push(path);
        }
    }
    assert_eq!(paths, ["left.value", "right.value"]);
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions.first(), definitions.get(1));
}

#[test]
fn opaque_reference_stops_at_declared_storage_boundary() {
    use hw_model_schema::{
        Endian, RecordSpec,
        ast::{Definition, Field, FieldType, RecordDefinition, Reference, ReferenceKind},
    };
    struct Opaque;
    fn opaque() -> Definition {
        Definition::opaque::<Opaque>(RecordSpec::new(4, 1, &[]))
    }
    let source = RecordDefinition::new(
        RecordSpec::new(4, 1, &[]),
        vec![Field::new(
            "payload",
            0,
            4,
            Endian::Little,
            FieldType::Reference(Reference::new("payload", ReferenceKind::Field, opaque)),
        )],
    );
    let schema = AnalyzedSchema::record("Envelope", source).unwrap();
    let root = hw_model_schema::traversal::record::definitions(&schema)
        .find(|v| v.context().unit().name == "Envelope")
        .unwrap();
    let field = root
        .context()
        .children()
        .find(|v| matches!(v.node(), Node::Field(_)))
        .unwrap();
    assert!(field.context().referent().is_none());
    let reference = field
        .context()
        .children()
        .find(|v| matches!(v.node(), Node::Reference(_)))
        .unwrap();
    let referent = reference.context().referent().unwrap();
    assert!(matches!(
        referent.node(),
        Node::Value(hw_model_schema::ast::value::Node::Opaque(_))
    ));
    assert_eq!(referent.context().unit().name, "Envelope");
    assert_eq!(referent.context().children().count(), 0);
    assert!(referent.context().referent().is_none());
}
