//! Checked layout formulas attach to the same semantic map declarations.
use hw_model_schema::analysis::{AnalyzedSchema, Bindings, Layout};
use hw_model_schema::{
    ast::{DefinitionId, LayoutExpr, SchemaSet},
    register_map, register_schema,
};
register_schema! { pub register Word { width:4, endian:Little, access:ReadOnly,
reset:Zero(0), writable_mask:0, fields:{} } }
register_map! { pub map Child(n: usize) { alignment:4, placements:[pub words:Word[n]] } }
register_map! { pub map Parent(n: usize) { alignment:16, placements:[
    pub head:Word, pub groups:Child(n)[2], pub tail:Word,
] } }
fn analyze(bindings: &Bindings) -> Result<AnalyzedSchema, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    Ok(AnalyzedSchema::from_graph(&set.resolve()?, bindings)?)
}
#[test]
fn symbolic_layout_retains_child_bindings_and_math_dependencies() {
    let analysis = analyze(&Bindings::new()).expect("valid analysis");
    let root = analysis.units().last().expect("parent");
    let Layout::Map(map) = &root.body else {
        panic!("map expected")
    };
    let plan = map.plan.as_ref().expect("checked plan");
    assert_eq!(plan.parameters(), ["n"]);
    assert_eq!(
        plan.placements()
            .iter()
            .map(|p| p.reference.name())
            .collect::<Vec<_>>(),
        ["head", "groups", "tail"]
    );
    let groups = &plan.placements().get(1).expect("groups placement");
    assert_eq!(groups.child_definition, DefinitionId::of::<Child>());
    assert_eq!(groups.arguments, [LayoutExpr::Parameter("n")]);
    assert_eq!(groups.child_alignment, Word::ALIGNMENT);
    for n in [1, 3, 5] {
        assert_eq!(
            groups.stride.evaluate(&["n"], &[n]).expect("closed stride"),
            n * Word::EXTENT
        );
        assert_eq!(
            groups
                .child_extent
                .evaluate(&["n"], &[n])
                .expect("closed child extent"),
            n * Word::EXTENT
        );
        assert_eq!(
            plan.evaluate(&[n]).expect("geometry").extent,
            Parent::new(n).expect("instance").extent()
        );
    }
}
#[test]
fn bound_layout_keeps_source_bindings_and_checked_constant_geometry() {
    for n in [1, 3, 5] {
        let mut bindings = Bindings::new();
        bindings.bind::<Parent>(&[n]);
        let analysis = analyze(&bindings).expect("valid analysis");
        let root = analysis.units().last().expect("parent");
        let Layout::Map(map) = &root.body else {
            panic!("map expected")
        };
        let plan = map.plan.as_ref().expect("checked plan");
        assert!(plan.parameters().is_empty());
        assert_eq!(map.arguments.as_deref(), Some([n].as_slice()));
        let expected = Parent::new(n).expect("instance");
        assert_eq!(plan.extent(), &LayoutExpr::Constant(expected.extent()));
        assert_eq!(
            plan.placements().get(1).expect("groups placement").stride,
            LayoutExpr::Constant(expected.groups().stride())
        );
        let hw_model_schema::ast::Node::Register(
            hw_model_schema::ast::register::Node::RegisterMap(
                hw_model_schema::ast::MapView::Analyzed { declaration, .. },
            ),
        ) = root.node().expect("node")
        else {
            panic!("source map expected")
        };
        assert_eq!(declaration.parameters(), &["n"]);
        assert_eq!(
            declaration
                .placements()
                .get(1)
                .expect("groups declaration")
                .arguments(),
            &[LayoutExpr::Parameter("n")]
        );
    }
}
