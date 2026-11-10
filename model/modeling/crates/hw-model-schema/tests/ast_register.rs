//! Register AST source ownership and canonical dependencies.
use hw_model_schema::ast::{DescribeSchema, Node, SchemaSet};
type TestResult = Result<(), Box<dyn std::error::Error>>;
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
    assert!(definition.node().children().all(|node| matches!(
        node,
        Node::Register(
            hw_model_schema::ast::register::Node::Attribute(_)
                | hw_model_schema::ast::register::Node::Reference(_)
        )
    )));
    assert_eq!(
        definition
            .node()
            .children()
            .filter(|node| matches!(
                node,
                Node::Register(hw_model_schema::ast::register::Node::Reference(_))
            ))
            .count(),
        1
    );
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
