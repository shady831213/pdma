//! Completed geometry is directly evaluable without placement dependency callbacks.
/// Expression-size oracle shared by the completed-layout checks.
pub mod completed_passes_support;
use completed_passes_support::Count;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Layout},
    ast::{LayoutExpr, SchemaSet},
    register_map, register_schema,
};
register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly, reset:Zero(0), writable_mask:0, fields:{} } }
register_map! { pub map Child(n: usize) { alignment:4, placements:[pub words:Word[n]] } }
register_map! { pub map Parent(n: usize) { alignment:16, placements:[pub head:Word, pub groups:Child(n)[2], pub tail:Word] } }
#[test]
fn completed_nested_formulas_evaluate_without_dependency_interpreter() {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let schema = AnalyzedSchema::from_graph(&set.resolve().unwrap(), &Bindings::new()).unwrap();
    let root = schema.units().last().unwrap();
    let Layout::Map(map) = &root.body else {
        panic!("map")
    };
    let plan = map.plan.as_ref().unwrap();
    for n in [1, 3, 5] {
        let expected = Parent::new(n).unwrap();
        assert_eq!(
            plan.extent().evaluate(&["n"], &[n]).unwrap(),
            expected.extent()
        );
        assert_eq!(
            plan.placements()
                .get(1)
                .expect("groups")
                .stride
                .evaluate(&["n"], &[n])
                .unwrap(),
            expected.groups().stride()
        );
        assert_eq!(plan.evaluate(&[n]).unwrap().extent, expected.extent());
    }
    assert!(plan.evaluate(&[0]).is_err());
    assert!(plan.evaluate(&[usize::MAX]).is_err());
    assert_eq!(
        plan.placements().first().expect("head").offset,
        LayoutExpr::Constant(0)
    );
}
#[test]
fn sequential_symbolic_maps_have_linear_extent_formulas() {
    use hw_model_schema::{
        Documentation,
        ast::{Definition, DescribeSchema, MapDefinition, MapPlacement},
    };
    struct Large;
    let source = MapDefinition::new(
        &["n"],
        4,
        None,
        (0..24)
            .map(|_| {
                MapPlacement::new(
                    "words",
                    Word::schema_definition,
                    vec![],
                    LayoutExpr::Parameter("n"),
                    None,
                    true,
                )
            })
            .collect(),
    );
    let expected = source.layout_plan().unwrap();
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register_map::<Large>(
        "Large",
        Documentation::EMPTY,
        source,
    ));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let Layout::Map(map) = &schema.units().last().unwrap().body else {
        panic!("map")
    };
    let plan = map.plan.as_ref().unwrap();
    let nodes = plan.extent().visit(&mut Count).unwrap();
    println!("24-placement extent expression nodes: {nodes}");
    assert!(nodes < 500);
    for n in [1, 3, 17] {
        assert_eq!(
            plan.evaluate(&[n]).unwrap(),
            expected.evaluate(&[n]).unwrap()
        );
    }
    assert!(!plan.constraints().is_empty());
}
#[test]
fn completed_layout_retains_documentation_references_and_bound_parameter_names() {
    use hw_model_schema::{
        Documentation,
        ast::{Definition, DefinitionId, DescribeSchema, MapDefinition, MapPlacement},
    };
    struct Documented;
    let docs = Documentation {
        short_description: Some("map docs"),
        description: Some("long map docs"),
    };
    let member_docs = Documentation {
        short_description: Some("placement docs"),
        description: None,
    };
    let source = MapDefinition::new(
        &[],
        4,
        None,
        vec![
            MapPlacement::new(
                "child",
                Child::schema_definition,
                vec![LayoutExpr::Constant(3)],
                LayoutExpr::Constant(1),
                None,
                false,
            )
            .with_documentation(member_docs),
        ],
    );
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register_map::<Documented>(
        "Documented",
        docs,
        source,
    ));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let root = schema.units().last().unwrap();
    assert_eq!(root.documentation, docs);
    assert_eq!(
        root.references.first().expect("child reference").target,
        schema
            .definition(DefinitionId::of::<Child>())
            .expect("child definition")
            .id
    );
    let Layout::Map(map) = &root.body else {
        panic!("map")
    };
    let placement = &map
        .plan
        .as_ref()
        .unwrap()
        .placements()
        .first()
        .expect("child placement");
    assert_eq!(placement.documentation, member_docs);
    assert_eq!(placement.child_parameters, ["n"]);
    assert_eq!(placement.arguments, [LayoutExpr::Constant(3)]);
}
#[test]
fn bound_formulas_are_constants_and_symbolic_overlap_is_retained() {
    use hw_model_schema::{
        Documentation,
        ast::{Definition, DescribeSchema, LayoutError, MapDefinition, MapPlacement},
    };
    struct Overlap;
    for n in [1, 3, 5] {
        let mut set = SchemaSet::new();
        set.add::<Parent>();
        let mut bindings = Bindings::new();
        bindings.bind::<Parent>(&[n]);
        let schema = AnalyzedSchema::analyze(set, &bindings).unwrap();
        let Layout::Map(map) = &schema.units().last().unwrap().body else {
            panic!("map")
        };
        let plan = map.plan.as_ref().unwrap();
        assert_eq!(
            plan.extent(),
            &LayoutExpr::Constant(Parent::new(n).unwrap().extent())
        );
        assert_eq!(plan.bindings(), Some([n].as_slice()));
        assert_eq!(plan.source_parameters(), ["n"]);
        assert!(plan.parameters().is_empty());
    }
    let source = MapDefinition::new(
        &["n"],
        4,
        None,
        vec![
            MapPlacement::new(
                "words",
                Word::schema_definition,
                vec![],
                LayoutExpr::Parameter("n"),
                None,
                true,
            ),
            MapPlacement::new(
                "tail",
                Word::schema_definition,
                vec![],
                LayoutExpr::Constant(1),
                Some(8),
                false,
            ),
        ],
    );
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register_map::<Overlap>(
        "Overlap",
        Documentation::EMPTY,
        source,
    ));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let Layout::Map(map) = &schema.units().last().unwrap().body else {
        panic!("map")
    };
    let plan = map.plan.as_ref().unwrap();
    assert_eq!(plan.evaluate(&[2]).unwrap().extent, 12);
    assert_eq!(plan.evaluate(&[3]), Err(LayoutError::Overlap));
}
#[test]
fn unbound_parameters_do_not_hide_statically_invalid_alignment() {
    use hw_model_schema::{
        Documentation,
        analysis::AnalysisError,
        ast::{Definition, DescribeSchema, LayoutError, MapDefinition, MapPlacement},
    };
    struct Misaligned;
    let source = MapDefinition::new(
        &["n"],
        4,
        None,
        vec![MapPlacement::new(
            "words",
            Word::schema_definition,
            vec![],
            LayoutExpr::Parameter("n"),
            Some(2),
            true,
        )],
    );
    let mut set = SchemaSet::new();
    set.add_definition(Definition::register_map::<Misaligned>(
        "Misaligned",
        Documentation::EMPTY,
        source,
    ));
    assert!(matches!(
        AnalyzedSchema::analyze(set, &Bindings::new()),
        Err(AnalysisError::Layout(LayoutError::Geometry))
    ));
}
