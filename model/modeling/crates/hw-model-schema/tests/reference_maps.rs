//! Child containment retains parameter scope, instance selection and register policy.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Layout},
    ast::{DefinitionBody, DefinitionId, LayoutExpr, SchemaSet},
    bits_schema, register_map, register_schema,
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
fn expansion_preserves_use_bindings_without_unrolling_banks() -> TestResult {
    let mut set = SchemaSet::new();
    set.add_map_instance::<Parent>("configured", &[2])?;
    let graph = set.resolve()?;
    let source = graph.get(DefinitionId::of::<Parent>()).ok_or("root")?;
    let DefinitionBody::RegisterMap(map) = source.body() else {
        return Err("map".into());
    };
    assert!(map.placements().first().ok_or("first")?.is_bank());
    assert_eq!(
        map.placements()
            .first()
            .ok_or("first")?
            .reference()
            .target()
            .id(),
        DefinitionId::of::<Child>()
    );
    let geometry = map.resolve(&[2])?;
    assert_eq!(geometry.extent, 20);
    assert_eq!(geometry.placements.get(1).ok_or("second")?.offset, 8);
    assert_eq!(
        graph.instances().first().ok_or("instance")?.geometry(),
        &geometry
    );
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let root = register::definitions(&schema)
        .find(|r| r.context().unit().name == "configured")
        .ok_or("configured")?;
    let mut scopes = Vec::new();
    let mut registers = 0;
    walk(root.context(), &mut scopes, &mut registers)?;
    assert_eq!(
        scopes,
        [
            (vec!["n"], Some(vec![2])),
            (vec!["m"], Some(vec![2])),
            (vec!["m"], Some(vec![3]))
        ]
    );
    assert_eq!(registers, 2);
    Ok(())
}
#[test]
fn bound_and_symbolic_references_share_definition_but_keep_distinct_layouts() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let mut bindings = Bindings::new();
    bindings.bind::<Parent>(&[3]);
    let schema = AnalyzedSchema::analyze(set, &bindings)?;
    let unit = schema
        .definition(DefinitionId::of::<Parent>())
        .ok_or("root")?;
    let Layout::Map(layout) = &unit.body else {
        return Err("map".into());
    };
    let plan = layout.plan.as_ref().ok_or("plan")?;
    assert_eq!(plan.bindings(), Some(&[3][..]));
    let first = plan.placements().first().ok_or("first")?;
    let second = plan.placements().get(1).ok_or("second")?;
    assert_eq!(first.child_definition, second.child_definition);
    assert_eq!(
        first
            .referent_layout
            .as_ref()
            .ok_or("first layout")?
            .bindings(),
        Some(&[3][..])
    );
    assert_eq!(
        second
            .referent_layout
            .as_ref()
            .ok_or("second layout")?
            .bindings(),
        Some(&[4][..])
    );
    assert_eq!(layout.instance.as_ref().ok_or("instance")?.extent, 28);
    Ok(())
}
#[test]
fn referenced_bits_keep_register_encoding_reset_and_permissions() -> TestResult {
    let mut set = SchemaSet::new();
    set.add::<Word>();
    let graph = set.resolve()?;
    let source = graph.get(DefinitionId::of::<Word>()).ok_or("register")?;
    let DefinitionBody::Register(register) = source.body() else {
        return Err("register".into());
    };
    assert_eq!(
        register.source().ok_or("bits")?.target().id(),
        DefinitionId::of::<Flags>()
    );
    assert_eq!(register.endian(), hw_model_schema::Endian::Big);
    assert_eq!(register.access(), hw_model_schema::Access::ReadOnly);
    assert_eq!(register.reset_value(), 1);
    assert_eq!(register.writable_mask(), 0);
    AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    Ok(())
}
type Scope = (Vec<&'static str>, Option<Vec<usize>>);
fn walk(ctx: &register::Context<'_>, scopes: &mut Vec<Scope>, registers: &mut usize) -> TestResult {
    if let Node::RegisterMap(_) = ctx.node() {
        scopes.push((
            ctx.declared_parameters().to_vec(),
            ctx.bindings().map(<[usize]>::to_vec),
        ));
    }
    if matches!(ctx.node(), Node::Register(_)) {
        *registers += 1;
        assert_eq!(ctx.unit().name, "configured");
        assert!(
            ctx.register_site().is_none(),
            "symbolic reference must not fabricate absolute register offset zero"
        );
    }
    if let Node::LayoutProperty(property) = ctx.node()
        && property.kind == hw_model_schema::ast::register::LayoutPropertyKind::Extent
    {
        assert_eq!(ctx.layout_property().ok_or("property")?.owner, None);
    }
    if let Some(target) = ctx.referent() {
        walk(target.context(), scopes, registers)?;
    }
    for child in ctx.children() {
        walk(child.context(), scopes, registers)?;
    }
    Ok(())
}
