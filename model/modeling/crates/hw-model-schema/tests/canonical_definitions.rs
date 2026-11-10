//! Source construction produces one graph of definitions and references.
use hw_model_schema::{
    ast::{DefinitionBody, DefinitionId, FieldType, SchemaSet},
    record_schema,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

record_schema! { pub record Packet { endian: Little, fields: {
    value: u64,
    flags: bits(u32) { fields: { enabled: 0..1, } },
    kind: enum(u32) { variants: { Empty = 0, Data = 7 } },
    payload: tagged_union { tag: self.kind, cases: { Empty {}, Data { number: u32 } } },
} } }
record_schema! { pub record Pair { endian: Little, fields: { first: Packet, second: Packet } } }

#[test]
fn defined_types_are_edges_and_builtin_values_stay_leaves() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let graph = set.resolve()?;
    let packet = graph.get(DefinitionId::of::<Packet>()).ok_or("packet")?;
    let DefinitionBody::Record(record) = packet.body() else {
        return Err("record".into());
    };
    assert!(matches!(
        record.fields().first().ok_or("value")?.ty(),
        FieldType::Scalar { .. }
    ));
    for field in record.fields().iter().skip(1) {
        assert!(
            matches!(field.ty(), FieldType::Reference(_)),
            "{} is a defined type",
            field.name()
        );
    }
    let payload = packet
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
    assert_eq!(target.references().len(), union.cases().len());
    Ok(())
}

#[test]
fn repeated_uses_share_the_canonical_target_including_its_dependencies() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    let pair = graph.get(DefinitionId::of::<Pair>()).ok_or("pair")?;
    for reference in pair.references() {
        let target = reference.target();
        assert_eq!(target.id(), DefinitionId::of::<Packet>());
        assert_eq!(target.references().len(), 3);
        for nested in target.references() {
            assert!(graph.get(nested.target().id()).is_some());
        }
    }
    Ok(())
}
