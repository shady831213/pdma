use super::{BoundContext, DomainNode, Edge, SourceRole, Visit};
use crate::ast::{SchemaGraph, record, register};
/// Record context backed directly by resolved source definitions.
/// Source contexts cannot be rebound to arbitrary declarations:
/// ```compile_fail
/// use hw_model_schema::{ast::{SchemaGraph, Definition, record::Node}, traversal::source::RecordContext};
/// fn forge<'a>(graph: &'a SchemaGraph, root: &'a Definition, node: Node<'a>) {
///     let _ = RecordContext::root(graph, root, node);
/// }
/// ```
pub type RecordContext<'a> = BoundContext<'a, record::Node<'a>, SchemaGraph>;
/// Register context backed directly by resolved source definitions.
pub type RegisterContext<'a> = BoundContext<'a, RegisterNode<'a>, SchemaGraph>;
/// Source register containment with a root boundary matching completed traversal.
#[derive(Debug, Clone, Copy)]
pub enum RegisterNode<'a> {
    /// Actual source definition, with no fabricated layout unit.
    Root(&'a crate::ast::Definition),
    /// Original source semantic node.
    Node(register::Node<'a>),
}
impl<'a> DomainNode<'a> for RegisterNode<'a> {
    fn children(self) -> std::vec::IntoIter<Self> {
        match self {
            Self::Root(definition) => source_register(definition.node())
                .map(Self::Node)
                .into_iter()
                .collect::<Vec<_>>()
                .into_iter(),
            Self::Node(node) => node
                .children()
                .map(Self::Node)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }
    fn edge(self, index: usize, child: Self) -> Edge {
        match (self, child) {
            (Self::Root(_), _) => Edge::Source(SourceRole::Declaration, 0),
            (_, Self::Node(node)) => super::source_edge::register(node, index),
            (_, Self::Root(_)) => Edge::Child(index),
        }
    }
}
const fn source_register(node: crate::ast::Node<'_>) -> Option<register::Node<'_>> {
    match node {
        crate::ast::Node::Register(node) => Some(node),
        crate::ast::Node::Value(node) => Some(register::Node::Value(node)),
        crate::ast::Node::Record(_) => None,
    }
}
/// Binds reusable record roots without running validation, layout or an exporter.
/// Unchanged source containment has the same IDs after semantic preparation.
pub fn record_roots(graph: &SchemaGraph) -> impl Iterator<Item = Visit<RecordContext<'_>>> {
    super::record::bind_roots(graph, graph.dependency_order())
}
/// Binds reusable register source roots before completed geometry exists.
///
/// Source attributes and declarations are available here. Layout-derived nodes
/// belong to a disjoint derived namespace in completed traversal.
/// Named map instances are not reusable source definitions and are prepared later.
pub fn register_roots(graph: &SchemaGraph) -> impl Iterator<Item = Visit<RegisterContext<'_>>> {
    graph.dependency_order().filter_map(|definition| {
        source_register(definition.node())?;
        Some(BoundContext::root(graph, definition, RegisterNode::Root(definition)).current())
    })
}
