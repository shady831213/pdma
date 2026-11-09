//! Child containment retains parameter scope, instance selection and register policy.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Layout, Pass, Pipeline, Resolve},
    ast::{DefinitionBody, DefinitionId, LayoutExpr, PlacementBody, SchemaSet},
    bits_schema, register_map, register_schema,
    transform::{Inline, Outline},
    traversal::register::{self, Node},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
bits_schema! { pub bits Flags(u32) { ready: 0..1, } }
register_schema! { pub register Word { width: 4, endian: Big, access: ReadOnly, reset: Zero(1), writable_mask: 0, fields: Flags } }
register_map! { pub map Child(m: usize) { alignment: 4, placements: [pub words: Word[m]] } }
struct Parent;
impl hw_model_schema::ast::DescribeSchema for Parent {
    fn schema_definition() -> hw_model_schema::ast::Definition {
        use hw_model_schema::ast::{Definition, MapDefinition, MapPlacement};
        Definition::register_map::<Self>(
            "Parent",
            hw_model_schema::Documentation::EMPTY,
            MapDefinition::new(
                &["n"],
                4,
                None,
                vec![
                    MapPlacement::new(
                        "one",
                        Child::schema_definition,
                        vec![LayoutExpr::Parameter("n")],
                        LayoutExpr::Constant(1),
                        None,
                        true,
                    ),
                    MapPlacement::new(
                        "two",
                        Child::schema_definition,
                        vec![LayoutExpr::Add(
                            Box::new(LayoutExpr::Parameter("n")),
                            Box::new(LayoutExpr::Constant(1)),
                        )],
                        LayoutExpr::Constant(1),
                        None,
                        false,
                    ),
                ],
            ),
        )
    }
}

#[test]
fn inline_maps_keep_two_child_bindings_and_bank_of_one() -> TestResult {
    // Given one formal child instantiated twice with different actual arguments.
    let mut set = SchemaSet::new();
    set.add_map_instance::<Parent>("configured", &[2])?;
    let graph = set.resolve()?;
    // When source references become owned child containment through the typed pipeline.
    let rewritten = Pipeline::new(graph)
        .run(Inline::new())?
        .run(Resolve)?
        .into_inner();
    let source = rewritten.get(DefinitionId::of::<Parent>()).ok_or("root")?;
    let DefinitionBody::RegisterMap(map) = source.body() else {
        return Err("map".into());
    };
    // Then both formal scopes and actual bindings remain, including explicit bank identity.
    assert_eq!(rewritten.roots(), &[DefinitionId::of::<Parent>()]);
    assert_eq!(
        rewritten
            .instances()
            .first()
            .ok_or("missing entry")?
            .arguments(),
        &[2]
    );
    assert_eq!(map.placements().len(), 2);
    assert!(map.placements().first().ok_or("missing entry")?.is_bank());
    assert!(!map.placements().get(1).ok_or("missing entry")?.is_bank());
    assert_eq!(
        map.placements().first().ok_or("missing entry")?.count(),
        &LayoutExpr::Constant(1)
    );
    assert!(matches!(
        map.placements().first().ok_or("missing entry")?.body(),
        PlacementBody::Inline(_)
    ));
    let geometry = map.resolve(&[2])?;
    assert_eq!(geometry.extent, 20);
    assert_eq!(geometry.placements.get(1).ok_or("missing entry")?.offset, 8);
    let schema = AnalyzedSchema::from_graph(&rewritten, &Bindings::new())?;
    let mut scopes = Vec::new();
    for root in register::roots(&schema).filter(|root| root.context().unit().name == "Parent") {
        walk(root.context(), &mut scopes);
    }
    assert_eq!(scopes, [vec!["n"], vec!["m"], vec!["m"]]);
    Ok(())
}

#[test]
fn register_bits_roundtrip_retains_encoding_and_policy() -> TestResult {
    // Given a register whose reusable bit source has independent identity.
    let mut set = SchemaSet::new();
    set.add::<Word>();
    // When expanding then outlining that source explicitly.
    let inline = Inline::new().run(set)?.resolve()?;
    let schema = AnalyzedSchema::from_graph(&inline, &Bindings::new())?;
    let unit = schema
        .definition(DefinitionId::of::<Word>())
        .ok_or("register")?;
    let Some(DefinitionBody::Register(register)) = &unit.declaration else {
        return Err("register".into());
    };
    assert!(register.source().is_none());
    assert!(register.inline_bits().is_some());
    let outlined = Outline::new().run(inline)?.resolve()?;
    // Then encoding, reset and read-only permissions stay on the register owner.
    let source = outlined.get(DefinitionId::of::<Word>()).ok_or("register")?;
    let DefinitionBody::Register(register) = source.body() else {
        return Err("register".into());
    };
    assert!(register.source().is_some());
    assert_eq!(register.endian(), hw_model_schema::Endian::Big);
    assert_eq!(register.access(), hw_model_schema::Access::ReadOnly);
    assert_eq!(register.reset_value(), 1);
    assert_eq!(register.writable_mask(), 0);
    AnalyzedSchema::from_graph(&outlined, &Bindings::new())?;
    Ok(())
}

#[test]
fn inline_preserves_root_bindings_and_completed_child_formals() -> TestResult {
    // Given a root-level binding selected by the original Rust identity.
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let mut bindings = Bindings::new();
    bindings.bind::<Parent>(&[3]);
    // When analyzed after expansion.
    let graph = Inline::new().run(set)?.resolve()?;
    let schema = AnalyzedSchema::from_graph(&graph, &bindings)?;
    // Then the root binding remains effective and nested declarations remain formal.
    let root = schema
        .definition(DefinitionId::of::<Parent>())
        .ok_or("root")?;
    let Layout::Map(layout) = &root.body else {
        return Err("map".into());
    };
    let plan = layout.plan.as_ref().ok_or("plan")?;
    assert_eq!(plan.bindings(), Some(&[3][..]));
    assert_eq!(
        plan.placements()
            .first()
            .ok_or("missing entry")?
            .inline_map
            .as_ref()
            .ok_or("child")?
            .parameters(),
        &["m"]
    );
    assert_eq!(layout.instance.as_ref().ok_or("instance")?.extent, 28);
    Ok(())
}
#[test]
fn map_roundtrip_recomputes_named_instance_child_identity() -> TestResult {
    // Given a named instance whose children will receive new outlined identities.
    let mut set = SchemaSet::new();
    set.add_map_instance::<Parent>("configured", &[2])?;
    let inline = Inline::new().run(set)?.resolve()?;
    // When outlining all owned children and resolving the transformed source again.
    let graph = Outline::new().run(inline)?.resolve()?;
    // Then named-instance geometry describes the transformed children, retaining binding and extent.
    let root = graph.get(DefinitionId::of::<Parent>()).ok_or("root")?;
    let DefinitionBody::RegisterMap(map) = root.body() else {
        return Err("map".into());
    };
    let geometry = map.resolve(&[2])?;
    assert_eq!(
        graph.instances().first().ok_or("missing entry")?.geometry(),
        &geometry
    );
    assert_eq!(geometry.extent, 20);
    assert_eq!(
        map.placements().first().ok_or("missing entry")?.arguments(),
        &[LayoutExpr::Parameter("n")]
    );
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}

fn walk(ctx: &register::Context<'_>, scopes: &mut Vec<Vec<&'static str>>) {
    if let Node::MapLayout(_) = ctx.node() {
        scopes.push(ctx.parameters().to_vec());
    }
    for child in ctx.children() {
        walk(child.context(), scopes);
    }
}
