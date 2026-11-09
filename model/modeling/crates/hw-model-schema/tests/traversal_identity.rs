//! Neutral occurrence identities are independent of backend features and visit order.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{SchemaSet, record::Node},
    record_schema,
    traversal::{Edge, record},
};
record_schema! { pub record Header { endian: Little, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { left: Header, right: Header } } }
#[test]
fn occurrence_ids_and_parents_survive_reordering() {
    let mut set = SchemaSet::new();
    set.add_record::<Pair>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let root = record::roots(&schema)
        .find(|v| v.context().unit().name == "Pair")
        .unwrap();
    assert_eq!(root.context().id(), &root.context().unit().id);
    let selected = schema
        .definition(hw_model_schema::ast::DefinitionId::of::<Pair>())
        .unwrap();
    assert_eq!(root.context().id(), &selected.id);
    assert!(schema.view("Pair").is_none());
    assert!(root.context().parent_id().is_none());
    assert!(root.context().parent().is_none());
    let children: Vec<_> = root.context().children().collect();
    assert!(
        children
            .iter()
            .all(|v| v.context().parent_id().as_ref() == Some(root.context().id()))
    );
    let fields: Vec<_> = children
        .into_iter()
        .filter(|v| matches!(v.node(), Node::Field(_)))
        .collect();
    assert_eq!(fields.len(), 2);
    let attributes: Vec<_> = fields.first().unwrap().context().children().collect();
    assert_eq!(attributes.len(), 4);
    assert!(
        attributes
            .iter()
            .all(|v| v.context().parent_id().as_ref()
                == Some(fields.first().unwrap().context().id()))
    );
    assert_ne!(
        attributes.first().unwrap().context().id(),
        attributes.get(1).unwrap().context().id()
    );
    let mut ids = Vec::new();
    for field in &fields {
        assert!(field.context().referent().is_none());
        let reference = field
            .context()
            .children()
            .find(|v| matches!(v.node(), Node::Reference(_)))
            .unwrap();
        let target = reference.context().referent().unwrap();
        assert_eq!(
            target.context().parent_id().as_ref(),
            Some(reference.context().id())
        );
        assert_eq!(target.context().id().path().last(), Some(&Edge::Referent));
        let parent = target.context().parent().unwrap();
        assert_eq!(parent.id(), reference.context().id());
        assert!(
            matches!(parent.node(), Node::Reference(r) if r.name() == field.context().field().unwrap().name())
        );
        assert!(core::ptr::eq(parent.schema(), core::ptr::from_ref(&schema)));
        assert_eq!(
            parent.referent().unwrap().context().id(),
            target.context().id()
        );
        let field_parent = parent.parent().unwrap();
        assert_eq!(field_parent.id(), field.context().id());
        assert_eq!(field_parent.parent().unwrap().id(), root.context().id());
        let value = target
            .context()
            .children()
            .find(|v| matches!(v.node(), Node::Field(_)))
            .unwrap();
        ids.push(value.context().id().clone());
    }
    assert_ne!(ids.first().unwrap(), ids.get(1).unwrap());
    for (field, expected) in fields.iter().zip(&ids).rev() {
        let reference = field
            .context()
            .children()
            .find(|v| matches!(v.node(), Node::Reference(_)))
            .unwrap();
        let target = reference.context().referent().unwrap();
        let value = target
            .context()
            .children()
            .find(|v| matches!(v.node(), Node::Field(_)))
            .unwrap();
        assert_eq!(value.context().id(), expected);
    }
}

use hw_model_schema::{register_map, register_schema};
register_schema! { pub register Word { width: 4, endian: Little, access: ReadWrite,
reset: Zero(0), writable_mask: 0xffff_ffff, fields: {} } }
register_map! { pub map Bank(count: usize) { alignment: 4, placements: [pub words: Word[count]] } }
#[test]
fn register_expressions_and_instances_have_unique_occurrence_ids() {
    use hw_model_schema::traversal::{NodeId, register};
    use std::collections::HashSet;
    fn walk(ctx: &register::Context<'_>, ids: &mut HashSet<NodeId>, expressions: &mut usize) {
        assert!(
            ids.insert(ctx.id().clone()),
            "duplicate occurrence {:?}",
            ctx.id()
        );
        if matches!(
            ctx.node(),
            register::Node::Value(hw_model_schema::ast::value::Node::Expression(_))
        ) {
            *expressions += 1;
        }
        for child in ctx.children().rev() {
            assert_eq!(child.context().parent_id().as_ref(), Some(ctx.id()));
            let parent = child.context().parent().unwrap();
            assert_eq!(parent.id(), ctx.id());
            assert_eq!(format!("{:?}", parent.node()), format!("{:?}", ctx.node()));
            assert_eq!(parent.register_location(), ctx.register_location());
            walk(child.context(), ids, expressions);
        }
    }
    fn instances(ctx: &register::Context<'_>, result: &mut Vec<(usize, NodeId)>) {
        if let register::Node::RegisterPlacement { view, .. } = ctx.node()
            && let Some(index) = view.segments.iter().find_map(|segment| segment.index)
        {
            result.push((index, ctx.id().clone()));
        }
        for child in ctx.children() {
            instances(child.context(), result);
        }
    }
    let mut set = SchemaSet::new();
    set.add::<Bank>();
    let mut bindings = Bindings::new();
    bindings.bind::<Bank>(&[3]);
    let schema = AnalyzedSchema::analyze(set, &bindings).unwrap();
    let root = register::roots(&schema)
        .find(|v| v.context().unit().name == "Bank")
        .unwrap();
    let mut occurrences = Vec::new();
    instances(root.context(), &mut occurrences);
    assert_eq!(
        occurrences.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        occurrences
            .iter()
            .map(|(_, id)| id)
            .collect::<HashSet<_>>()
            .len(),
        3
    );
    let mut ids = HashSet::new();
    let mut expressions = 0;
    walk(root.context(), &mut ids, &mut expressions);
    assert!(expressions >= 6);
    assert!(ids.len() > 30);
    let root_again = register::roots(&schema)
        .find(|v| v.context().unit().name == "Bank")
        .unwrap();
    let mut repeat = HashSet::new();
    walk(root_again.context(), &mut repeat, &mut 0);
    assert_eq!(ids, repeat);
}
