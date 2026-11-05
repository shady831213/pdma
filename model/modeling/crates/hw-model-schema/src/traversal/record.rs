use super::{BoundContext, DomainNode, Edge, Schema, Visit};
use crate::{analysis::AnalyzedSchema, ast::record::Node};
/// Record-domain traversal context, shared by passes and emitters.
pub type Context<'a> = BoundContext<'a, Node<'a>>;
impl<'a> DomainNode<'a> for Node<'a> {
    fn children(self) -> std::vec::IntoIter<Self> {
        self.children()
    }
}
impl<'a, S: Schema> BoundContext<'a, Node<'a>, S> {
    /// Follows this reference occurrence while retaining the selected root and ancestry.
    pub fn referent(&self) -> Option<Visit<Self>> {
        let target = self.resolve(self.node().reference()?)?;
        let node = match S::declaration(target)?.node() {
            crate::ast::Node::Record(node) => node,
            crate::ast::Node::Value(node) => Node::Value(node),
            crate::ast::Node::Register(_) => return None,
        };
        Some(Visit::new(self.descendant(node, Edge::Referent)))
    }
    /// Enclosing bit-field name.
    pub fn bit(&self) -> Option<&'static str> {
        self.lineage().find_map(|node| match node {
            Node::Value(crate::ast::value::Node::BitField(v)) => Some(v.name()),
            _ => None,
        })
    }
    /// Enclosing logical field.
    pub fn field(&self) -> Option<&'a crate::ast::Field> {
        self.lineage().find_map(|node| match node {
            Node::Field(v) => Some(v),
            _ => None,
        })
    }
}
/// Binds record roots independently of backend or artifact-generation features.
/// Non-record domains are omitted; opaque storage remains an opaque value node.
pub fn roots(schema: &AnalyzedSchema) -> impl Iterator<Item = Visit<Context<'_>>> {
    bind_roots(schema, schema.units())
}
pub(crate) fn bind_roots<'a, S: Schema>(
    schema: &'a S,
    roots: impl Iterator<Item = &'a S::Root>,
) -> impl Iterator<Item = Visit<BoundContext<'a, Node<'a>, S>>> {
    roots.filter_map(|root| {
        let node = match S::declaration(root)?.node() {
            crate::ast::Node::Record(v) => v,
            crate::ast::Node::Value(v) => Node::Value(v),
            crate::ast::Node::Register(_) => return None,
        };
        Some(BoundContext::root(schema, root, node).current())
    })
}
