//! Source-stage passes and analyzed traversal share occurrence identities.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::{Reference, SchemaGraph, SchemaSet, record::Node},
    record_schema,
    traversal::{BoundContext, NodeId, Schema, record, source},
};
use std::collections::BTreeMap;
record_schema! { pub record Header { endian: Little, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { left: Header, right: Header } } }
fn collect<S: Schema>(
    ctx: &BoundContext<'_, Node<'_>, S>,
    out: &mut BTreeMap<String, NodeId>,
) -> Result<(), &'static str> {
    let mut path = vec![
        ctx.id()
            .source_path()
            .ok_or("source root missing")?
            .rsplit("::")
            .next()
            .ok_or("root name missing")?,
    ];
    path.extend(ctx.ancestors().iter().filter_map(|node| match node {
        Node::Field(field) => Some(field.name()),
        _ => None,
    }));
    if let Node::Field(field) = ctx.node() {
        path.push(field.name());
        out.insert(path.join("."), ctx.id().clone());
    }
    for child in ctx.children() {
        assert_eq!(child.context().parent_id().as_ref(), Some(ctx.id()));
        collect(child.context(), out)?;
    }
    if let Some(referent) = ctx.referent() {
        assert_eq!(referent.context().parent_id().as_ref(), Some(ctx.id()));
        collect(referent.context(), out)?;
    }
    Ok(())
}
struct FieldIds;
impl Pass<&SchemaGraph> for FieldIds {
    type Output = BTreeMap<String, NodeId>;
    type Error = &'static str;
    fn run(self, graph: &SchemaGraph) -> Result<Self::Output, Self::Error> {
        let mut ids = BTreeMap::new();
        for root in source::record_roots(graph) {
            assert!(core::ptr::eq(root.context().schema(), graph));
            assert_eq!(
                Some(root.context().owner().id().path()),
                root.context().id().source_path()
            );
            collect(root.context(), &mut ids)?;
        }
        Ok(ids)
    }
}
#[test]
fn pre_layout_pass_and_completed_schema_share_occurrence_ids() {
    let mut set = SchemaSet::new();
    set.add_record::<Pair>();
    let graph = set.resolve().unwrap();
    let before = FieldIds.run(&graph).unwrap();
    assert_eq!(
        before.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "Header.value",
            "Pair.left",
            "Pair.left.value",
            "Pair.right",
            "Pair.right.value"
        ]
    );
    assert_ne!(
        before.get("Pair.left.value"),
        before.get("Pair.right.value")
    );
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new()).unwrap();
    let mut after = BTreeMap::new();
    for root in record::definitions(&schema) {
        collect(root.context(), &mut after).unwrap();
    }
    assert_eq!(before, after);
}

use hw_model_schema::{register_map, register_schema};
register_schema! { pub register Word { width: 4, endian: Little, access: ReadWrite,
reset: Zero(0), writable_mask: 0xffff_ffff, fields: { payload: 0..32 } } }
register_map! { pub map Child(n: usize) { alignment: 4, placements: [pub words: Word[n]] } }
register_map! { pub map Parent(n: usize) { alignment: 4, placements: [pub child: Child(n)] } }
#[test]
fn register_source_identity_survives_derived_layout_without_aliasing() {
    use hw_model_schema::traversal::{Context, Edge, Visit, register};
    use std::collections::HashMap;
    fn ids<C: Context>(
        visit: &Visit<C>,
        out: &mut HashMap<NodeId, String>,
        label: impl Fn(C::Node) -> String + Copy,
    ) {
        assert!(
            out.insert(visit.context().id().clone(), label(visit.node()))
                .is_none()
        );
        for child in visit.context().children() {
            ids(&child, out, label);
        }
    }
    fn source_label(node: source::RegisterNode<'_>) -> String {
        use hw_model_schema::ast::register::Node as N;
        match node {
            source::RegisterNode::Root(root) => format!("root:{}", root.name()),
            source::RegisterNode::Node(node) => match node {
                N::Register(v) => format!("register:{}", v.name()),
                N::RegisterMap(_) => "map".into(),
                N::RegisterField(v) => format!("field:{}", v.name()),
                N::Placement(_) => format!(
                    "placement:{:?}",
                    node.children().find_map(N::reference).map(Reference::name)
                ),
                N::Reference(v) => format!("reference:{}", v.name()),
                N::Parameter(v) => format!("parameter:{}", v.name),
                N::Argument(v) => format!("argument:{:?}", v.parameter),
                N::Attribute(v) => format!("attribute:{v:?}"),
                N::Value(v) => format!("value:{v:?}"),
                N::LayoutProperty(_) => "derived".into(),
            },
        }
    }
    fn completed_label(node: register::Node<'_>) -> String {
        use register::Node as N;
        match node {
            N::Root(root) => format!("root:{}", root.name),
            N::Register(v) => format!("register:{}", v.name()),
            N::RegisterMap(_) => "map".into(),
            N::RegisterField(v) => format!("field:{}", v.name()),
            N::Placement(v) => format!("placement:{:?}", Some(v.source.name())),
            N::Reference(v) => format!("reference:{}", v.name()),
            N::Parameter(v) => format!("parameter:{}", v.name),
            N::Argument(v) => format!("argument:{:?}", v.parameter),
            N::Attribute(v) => format!("attribute:{v:?}"),
            N::Value(v) => format!("value:{v:?}"),
            N::RegisterPlacement { .. }
            | N::Bank(_)
            | N::BankProperty(_)
            | N::LayoutProperty(_) => "derived".into(),
        }
    }
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let graph = set.resolve().unwrap();
    let mut source_ids = HashMap::new();
    for root in source::register_roots(&graph) {
        ids(&root, &mut source_ids, source_label);
    }
    let mut bindings = Bindings::new();
    bindings.bind::<Parent>(&[3]);
    let schema = AnalyzedSchema::from_graph(&graph, &bindings).unwrap();
    let mut completed_ids = HashMap::new();
    for root in register::definitions(&schema) {
        ids(&root, &mut completed_ids, completed_label);
    }
    let derived: Vec<_> = completed_ids
        .iter()
        .filter(|(id, _)| {
            id.path()
                .iter()
                .any(|edge| matches!(edge, Edge::Derived(_)))
        })
        .collect();
    assert!(!derived.is_empty());
    assert!(derived.iter().all(|(id, _)| !source_ids.contains_key(*id)));
    let shared: Vec<_> = completed_ids
        .iter()
        .filter(|(id, _)| {
            !id.path()
                .iter()
                .any(|edge| matches!(edge, Edge::Derived(_)))
        })
        .collect();
    assert!(shared.len() >= 18);
    assert!(
        shared
            .iter()
            .all(|(id, label)| source_ids.get(*id) == Some(*label))
    );
    for (id, _) in &shared {
        if let Some(parent) = id.parent() {
            assert!(completed_ids.contains_key(&parent));
        }
    }
}
