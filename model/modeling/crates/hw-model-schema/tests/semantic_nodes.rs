//! Semantic traversal exposes source bindings and mathematical layout dependencies.
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::{self, LayoutExpr, PlanValue, SchemaSet},
    register_map, register_schema,
};
register_schema! { pub register Word { width: 4, endian: Little, access: ReadWrite,
reset: Zero(0), writable_mask: 0xffff_ffff, fields: {} } }
register_map! { pub map Child(words: usize) { alignment: 4, placements: [pub words: Word[words]] } }
register_map! { pub map Parent(count: usize) { alignment: 4, placements: [pub child: Child(count)] } }

fn walk(node: ast::Node<'_>, out: &mut Vec<String>) {
    match node {
        ast::Node::Register(ast::register::Node::Parameter(p)) => {
            out.push(format!("parameter:{}:{:?}", p.name, p.binding));
        }
        ast::Node::Register(ast::register::Node::Argument(a)) => {
            out.push(format!("argument:{:?}", a.parameter));
        }
        ast::Node::Register(ast::register::Node::LayoutProperty(p)) => {
            out.push(format!("property:{:?}", p.kind));
        }
        ast::Node::Value(ast::value::Node::Expression(LayoutExpr::Parameter(p))) => {
            out.push(format!("use:{p}"));
        }
        ast::Node::Value(ast::value::Node::Expression(LayoutExpr::Value(
            PlanValue::ChildExtent,
        ))) => out.push("child-extent".into()),
        _ => {}
    }
    for child in node.children() {
        walk(child, out);
    }
}
#[test]
fn source_walk_retains_parameters_constructor_bindings_and_expression_uses() {
    use ast::DescribeSchema;
    let definition = Parent::schema_definition();
    let mut seen = Vec::new();
    walk(definition.node(), &mut seen);
    assert!(seen.contains(&"parameter:count:None".into()));
    assert!(seen.contains(&"argument:Some(\"words\")".into()));
    assert!(seen.contains(&"use:count".into()));
}
#[test]
fn analyzed_map_attaches_math_to_same_source_nodes() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let graph = set.resolve()?;
    let mut bindings = Bindings::new();
    bindings.bind::<Parent>(&[3]);
    let analysis = AnalyzedSchema::from_graph(&graph, &bindings)?;
    let unit = analysis
        .units()
        .find(|u| u.name == "Parent")
        .ok_or("parent missing")?;
    let mut seen = Vec::new();
    walk(unit.node()?, &mut seen);
    assert!(seen.contains(&"parameter:count:Some(3)".into()));
    assert!(seen.contains(&"argument:Some(\"words\")".into()));
    assert!(seen.contains(&"use:count".into()));
    assert!(seen.contains(&"property:Stride".into()));
    let symbolic = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let symbolic_unit = symbolic
        .units()
        .find(|u| u.name == "Parent")
        .ok_or("symbolic parent missing")?;
    let mut symbolic_seen = Vec::new();
    walk(symbolic_unit.node()?, &mut symbolic_seen);
    assert!(!symbolic_seen.contains(&"child-extent".into()));
    Ok(())
}

mod semantic_callbacks;
#[test]
fn record_domain_keeps_scalar_constraints_as_source_nodes() {
    use hw_model_schema::{Endian, FieldDefinition, RecordSpec};
    let constant = ast::Constant {
        symbol: "MAGIC",
        value: 7,
    };
    let field = ast::Field::new(
        "tag",
        0,
        4,
        Endian::Little,
        ast::FieldType::Scalar {
            definition: FieldDefinition::new(hw_model_schema::ScalarKind::U32),
            constant: Some(constant),
        },
    );
    let node = ast::record::Node::Field(&field);
    assert!(
        node.children()
            .any(|n| matches!(n, ast::record::Node::Constant(c) if *c == constant))
    );
    assert!(node.children().any(|n| matches!(
        n,
        ast::record::Node::Attribute(ast::record::Attribute::Size(4))
    )));
    let opaque = RecordSpec::named("Opaque", 8, 4, &[], &[]);
    assert!(matches!(
        ast::Definition::opaque::<u64>(opaque).node(),
        ast::Node::Value(ast::value::Node::Opaque(_))
    ));
}
#[test]
fn register_access_and_shape_validation_are_shared_semantics() {
    use hw_model_schema::{Access, Endian, RegisterDefinition};
    #[derive(Clone, Copy)]
    struct Identity;
    assert!(Access::ReadOnly.readable());
    assert!(!Access::ReadOnly.writable());
    assert!(Access::ReadWrite.readable() && Access::ReadWrite.writable());
    assert!(!Access::WriteOnly.readable());
    assert!(Access::WriteOnly.writable());
    assert!(
        RegisterDefinition::<Identity>::new("good", 4, Endian::Little)
            .validate_shape()
            .is_ok()
    );
    assert!(
        RegisterDefinition::<Identity>::new("bad", 16, Endian::Little)
            .validate_shape()
            .is_err()
    );
    assert!(
        RegisterDefinition::<Identity>::new("bad", 4, Endian::Little)
            .with_alignment(3)
            .validate_shape()
            .is_err()
    );
    let mut schemas = SchemaSet::new();
    schemas.add_definition(ast::Definition::register::<Identity, Identity>(
        RegisterDefinition::new("bad", 16, Endian::Little),
    ));
    let graph = schemas.resolve().expect("structural graph");
    assert!(matches!(
        AnalyzedSchema::from_graph(&graph, &Bindings::new()),
        Err(hw_model_schema::analysis::AnalysisError::Register { .. })
    ));
}
