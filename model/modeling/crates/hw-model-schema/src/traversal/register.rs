//! Completed register nodes and immutable semantic context.
mod bank;
mod context;
mod identity;
mod node;
pub use bank::{Bank, BankProperty};
pub use context::{Context, LayoutProperty, LayoutPropertyKind, RegisterSite};
pub use node::Node;
impl<'a> super::DomainNode<'a> for Node<'a> {
    fn children(self) -> std::vec::IntoIter<Self> {
        self.children()
    }
    fn edge(self, index: usize, child: Self) -> super::Edge {
        identity::edge(self, index, child)
    }
}

/// Binds register-domain roots for traversal independently of any backend.
pub fn roots(
    schema: &crate::analysis::AnalyzedSchema,
) -> impl Iterator<Item = crate::traversal::Visit<Context<'_>>> {
    schema
        .units()
        .filter(|unit| supports(unit))
        .map(|unit| Context::root(schema, unit, Node::Root(unit)).current())
}

pub(crate) const fn supports(unit: &crate::analysis::Unit) -> bool {
    !matches!(
        unit.declaration,
        Some(crate::ast::DefinitionBody::Record(_) | crate::ast::DefinitionBody::Union(_))
    )
}
