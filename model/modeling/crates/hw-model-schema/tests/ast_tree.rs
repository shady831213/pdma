//! Canonical source containment remains independent of physical storage lowering.
use hw_model_schema::{
    ast::{DescribeSchema, FieldType, Node, SchemaSet},
    record_schema,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Leaf { endian: Little, fields: { number: u32 } } }
record_schema! { pub record Source { endian: Little, fields: {
    magic: u32 { const: Source::record_size },
    kind: enum(u32) { variants: { Empty = 0, Data = 3 } },
    payload: tagged_union { tag: self.kind, cases: { Empty {}, Data { number: u32, flags: SharedBits } } },
    right: Leaf,
    left: Leaf,
} } }

#[test]
fn source_tree_retains_union_constant_order_and_repeated_references() -> TestResult {
    // Given a DSL record whose inline union codec lowers to raw words.
    let definition = <Source as DescribeSchema>::schema_definition();
    let Node::Record(record) = definition.node() else {
        return Err("expected record".into());
    };
    // When walking canonical containment, source names and constant constraints remain.
    let names: Vec<_> = record
        .fields()
        .iter()
        .map(hw_model_schema::ast::Field::name)
        .collect();
    assert_eq!(names, ["magic", "kind", "payload", "right", "left"]);
    assert!(
        record
            .spec()
            .fields()
            .iter()
            .any(|field| field.name() == "word0")
    );
    let FieldType::Scalar {
        constant: Some(constant),
        ..
    } = record
        .fields()
        .first()
        .ok_or("missing constant field")?
        .ty()
    else {
        return Err("missing constant".into());
    };
    assert_eq!(
        (constant.symbol, constant.value),
        ("Source :: record_size", 24)
    );
    let field = definition
        .node()
        .children()
        .nth(2)
        .ok_or("missing payload")?;
    let Node::Union(union) = field.children().next().ok_or("missing union")? else {
        return Err("expected union".into());
    };
    assert_eq!(union.selector(), Some("self.kind"));
    assert_eq!(
        union
            .cases()
            .iter()
            .map(|case| (case.name(), case.discriminant()))
            .collect::<Vec<_>>(),
        [("Empty", Some(0)), ("Data", Some(3))]
    );
    // Then repeated edges remain, but the referenced definition is collected once.
    let case = Node::Union(union)
        .children()
        .nth(1)
        .ok_or("missing Data case")?;
    let case_names: Vec<_> = case
        .children()
        .filter_map(|node| match node {
            Node::Field(field) => Some(field.name()),
            _ => None,
        })
        .collect();
    assert_eq!(case_names, ["number", "flags"]);
    assert_eq!(case.references().count(), 1);
    assert_eq!(
        definition
            .node()
            .references()
            .map(hw_model_schema::ast::Reference::name)
            .collect::<Vec<_>>(),
        ["flags", "right", "left"]
    );
    let mut set = SchemaSet::new();
    set.add::<Source>();
    let graph = set.resolve()?;
    assert_eq!(graph.definitions().count(), 3);
    assert_eq!(
        graph
            .definitions()
            .filter(|definition| definition.name() == "Leaf")
            .count(),
        1
    );
    let mut children: Vec<_> = definition.node().children().collect();
    children.sort_by_key(|node| match node {
        Node::Field(field) => field.name(),
        _ => "",
    });
    assert!(matches!(children.first(), Some(Node::Field(field)) if field.name() == "kind"));
    assert_eq!(
        record
            .fields()
            .iter()
            .map(hw_model_schema::ast::Field::name)
            .collect::<Vec<_>>(),
        names
    );
    println!(
        "source fields={names:?}; selector={:?}; cases=Empty:0,Data:3; references=3; inline Data fields=number,flags; unique Leaf=1",
        union.selector()
    );
    Ok(())
}

record_schema! { pub record InlineBits { alignment: 8, endian: Big, fields: {
    flags: bits(u32) { fields: {
        mode: 0..2 => enum { variants: { Off = 0, On = 1 } },
        enabled: 3..4,
    } },
} } }

#[test]
fn inline_bits_and_enumeration_are_containment_nodes() -> TestResult {
    // Given inline bit and enum declarations.
    let definition = <InlineBits as DescribeSchema>::schema_definition();
    let Node::Record(record) = definition.node() else {
        return Err("expected record".into());
    };
    assert_eq!(record.declared_alignment(), Some(8));
    assert_eq!(record.endian(), hw_model_schema::Endian::Big);
    // When traversing original containment.
    let field = definition.node().children().next().ok_or("missing field")?;
    let bits = field.children().next().ok_or("missing bits")?;
    assert!(matches!(bits, Node::Bits(_)));
    let bit = bits.children().next().ok_or("missing bit field")?;
    let enumeration = bit.children().next().ok_or("missing enum")?;
    assert!(matches!(enumeration, Node::Enum(_)));
    // Then variants retain source codes and no definition edges are fabricated.
    let variants: Vec<_> = enumeration
        .children()
        .filter_map(|node| match node {
            Node::Variant(variant) => Some((variant.name(), variant.value())),
            _ => None,
        })
        .collect();
    assert_eq!(variants, [("Off", 0), ("On", 1)]);
    assert_eq!(definition.node().references().count(), 0);
    Ok(())
}

#[cfg(feature = "discovery")]
#[test]
fn manual_and_discovered_roots_share_one_identity_collector() -> TestResult {
    // Given generated discovery and independently authored source nodes.
    struct Manual;
    let mut set = hw_model_schema::ast::SchemaRegistry::discover().select_module(module_path!());
    let field = hw_model_schema::ast::Field::new(
        "leaf",
        0,
        4,
        hw_model_schema::Endian::Little,
        FieldType::Reference(hw_model_schema::ast::Reference::new(
            "leaf",
            hw_model_schema::ast::ReferenceKind::Field,
            <Leaf as DescribeSchema>::schema_definition,
        )),
    );
    set.add_definition(hw_model_schema::ast::Definition::record::<Manual>(
        hw_model_schema::RecordSpec::named("Manual", 4, 4, &[], &[]),
        vec![field],
    ));
    set.add::<Leaf>();
    // When resolving all roots through the shared collector.
    let graph = set.resolve()?;
    // Then the reused definition is unique and appears before its manual dependent.
    assert_eq!(
        graph
            .definitions()
            .filter(|definition| definition.name() == "Leaf")
            .count(),
        1
    );
    let order: Vec<_> = graph
        .dependency_order()
        .map(hw_model_schema::ast::Definition::name)
        .collect();
    let leaf = order
        .iter()
        .position(|name| *name == "Leaf")
        .ok_or("missing Leaf")?;
    let manual = order
        .iter()
        .position(|name| *name == "Manual")
        .ok_or("missing Manual")?;
    assert!(leaf < manual);
    Ok(())
}

hw_model_schema::bits_schema! { pub bits SharedBits(u32) { flag: 0..1 } }
hw_model_schema::register_schema! { pub register SharedRegister {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 1,
    fields: SharedBits,
} }

#[test]
fn register_bits_reference_is_an_edge_not_duplicated_containment() -> TestResult {
    // Given a register reuses an independently declared bits definition.
    let definition = <SharedRegister as DescribeSchema>::schema_definition();
    // When examining its original source, the reference remains explicit.
    assert_eq!(definition.node().children().count(), 0);
    let references: Vec<_> = definition.node().references().collect();
    assert_eq!(references.len(), 1);
    assert_eq!(
        references
            .first()
            .ok_or("missing bits reference")?
            .target()
            .name(),
        "SharedBits"
    );
    // Then the common collector includes the bits declaration once.
    let mut set = SchemaSet::new();
    set.add::<SharedRegister>();
    let graph = set.resolve()?;
    assert_eq!(graph.definitions().count(), 2);
    Ok(())
}
