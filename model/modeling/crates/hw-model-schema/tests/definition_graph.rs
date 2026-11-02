//! Definition graph traversal is independent of automatic discovery.
use hw_model_schema::ast::{ReferenceKind, SchemaSet};
use hw_model_schema::{record_schema, union_schema};
type TestResult = Result<(), Box<dyn std::error::Error>>;

record_schema! {
    pub record Leaf { alignment: 4, endian: Little, fields: { value: u32 } }
}
record_schema! {
    pub record Inline { alignment: 4, endian: Little, fields: {
        kind: enum(u32) { variants: { Empty = 0, Value = 7 } },
        body: tagged_union { tag: self.kind, cases: { Empty {}, Value { number: u32 } } },
    } }
}

#[test]
fn inline_union_exposes_its_selector_and_case_fields() -> TestResult {
    // Given: a record embeds inline union alternatives rather than named codec types.
    let mut schemas = SchemaSet::new();
    schemas.add_record::<Inline>();
    // When: references are collected.
    let graph = schemas.resolve()?;
    // Then: an alternative retains the field shape and the original wire tag.
    let record = graph
        .definitions()
        .find(|node| node.name() == "Inline")
        .ok_or("missing record")?;
    let field = record
        .node()
        .children()
        .filter(|node| {
            matches!(
                node,
                hw_model_schema::ast::Node::Record(hw_model_schema::ast::record::Node::Field(_))
            )
        })
        .nth(1)
        .ok_or("missing union field")?;
    let hw_model_schema::ast::Node::Record(hw_model_schema::ast::record::Node::Union(union)) =
        field
            .children()
            .find(|node| {
                matches!(
                    node,
                    hw_model_schema::ast::Node::Record(hw_model_schema::ast::record::Node::Union(
                        _
                    ))
                )
            })
            .ok_or("missing union")?
    else {
        return Err("expected union".into());
    };
    let alternative = union.cases().get(1).ok_or("missing Value case")?;
    assert_eq!(alternative.discriminant(), Some(7));
    let hw_model_schema::ast::CaseBody::Inline(spec) = alternative.body() else {
        return Err("expected inline case fields".into());
    };
    let field = spec.fields().first().ok_or("missing case field")?;
    assert_eq!(field.name(), "number");
    assert_eq!(field.width(), 4);
    Ok(())
}
record_schema! {
    pub record Pair { alignment: 4, endian: Little, fields: { left: Leaf, right: Leaf } }
}
union_schema! {
    pub union Packet { cases: { Pair(Pair), Leaf(Leaf) } }
}
record_schema! {
    pub record Tagged { alignment: 4, endian: Little, fields: {
        kind: enum(u32) { variants: { Pair = 1, Leaf = 2 } },
        body: tagged_union { tag: self.kind, cases: { Pair(Pair), Leaf(Leaf) } },
    } }
}

#[test]
fn tagged_union_preserves_case_references_and_wire_discriminants() -> TestResult {
    // Given: only the enclosing record is selected.
    let mut schemas = SchemaSet::new();
    schemas.add_record::<Tagged>();
    // When: all type references are traversed.
    let graph = schemas.resolve()?;
    // Then: the branch graph retains the selector, codes and reused records.
    let record = graph
        .definitions()
        .find(|node| node.name() == "Tagged")
        .ok_or("missing record")?;
    let field = record
        .node()
        .children()
        .filter(|node| {
            matches!(
                node,
                hw_model_schema::ast::Node::Record(hw_model_schema::ast::record::Node::Field(_))
            )
        })
        .nth(1)
        .ok_or("missing union field")?;
    let hw_model_schema::ast::Node::Record(hw_model_schema::ast::record::Node::Union(union)) =
        field
            .children()
            .find(|node| {
                matches!(
                    node,
                    hw_model_schema::ast::Node::Record(hw_model_schema::ast::record::Node::Union(
                        _
                    ))
                )
            })
            .ok_or("missing union")?
    else {
        return Err("expected union".into());
    };
    let codes: Vec<_> = union
        .cases()
        .iter()
        .map(hw_model_schema::ast::Case::discriminant)
        .collect();
    assert_eq!(codes, [Some(1), Some(2)]);
    assert!(graph.definitions().any(|node| node.name() == "Pair"));
    assert!(graph.definitions().any(|node| node.name() == "Leaf"));
    Ok(())
}

#[test]
fn manual_union_root_collects_transitive_definitions_and_keeps_both_field_edges() -> TestResult {
    // Given: only the union is explicitly selected, without a registry.
    let mut schemas = SchemaSet::new();
    schemas.add_record::<Packet>();
    // When: dependencies are resolved.
    let graph = schemas.resolve()?;
    // Then: shared definitions are unique, but their distinct references remain.
    let names: Vec<_> = graph
        .definitions()
        .map(hw_model_schema::ast::Definition::name)
        .collect();
    assert_eq!(names, ["Leaf", "Packet", "Pair"]);
    let pair = graph
        .definitions()
        .find(|node| node.name() == "Pair")
        .ok_or("missing pair")?;
    let references = pair.references();
    let [left, right] = references.as_slice() else {
        return Err("expected two edges".into());
    };
    assert_eq!(left.name(), "left");
    assert_eq!(right.name(), "right");
    assert_eq!(left.kind(), ReferenceKind::Field);
    assert_eq!(left.target().id(), right.target().id());
    assert_eq!(graph.roots().len(), 1);
    Ok(())
}

#[test]
fn adding_an_existing_dependency_is_idempotent_and_marks_it_as_a_root() -> TestResult {
    // Given: the parent and its child are explicitly selected more than once.
    let mut schemas = SchemaSet::new();
    schemas.add_record::<Pair>();
    schemas.add_record::<Leaf>();
    schemas.add_record::<Leaf>();
    // When: the set is resolved.
    let graph = schemas.resolve()?;
    // Then: two definitions and two roots remain.
    assert_eq!(graph.definitions().count(), 2);
    assert_eq!(graph.roots().len(), 2);
    Ok(())
}
