use super::Node;
use crate::traversal::{Edge, SourceRole, source_edge};
pub(super) const fn edge(parent: Node<'_>, index: usize, child: Node<'_>) -> Edge {
    if matches!(parent, Node::Argument(_)) {
        return Edge::Derived(index);
    }
    if let Node::Register(register) = parent
        && register.source().is_some()
        && matches!(child, Node::RegisterField(_))
    {
        return Edge::Derived(index);
    }
    let role = match child {
        Node::MapLayout(_) | Node::Register(_) => SourceRole::Declaration,
        Node::Value(_) if matches!(parent, Node::Root(_)) => SourceRole::Declaration,
        Node::Parameter(_) => SourceRole::Parameter,
        Node::Placement(_) => SourceRole::Placement,
        Node::Reference(_) => SourceRole::Reference,
        Node::Argument(_) => SourceRole::Argument,
        Node::RegisterField(_) => SourceRole::Field,
        Node::Attribute(value) => source_edge::attribute(value),
        Node::Value(_) => SourceRole::Value,
        Node::Root(_)
        | Node::RegisterMap(_)
        | Node::RegisterPlacement { .. }
        | Node::Bank(_)
        | Node::BankProperty(_)
        | Node::LayoutProperty(_) => {
            return Edge::Derived(index);
        }
    };
    Edge::Source(role, 0)
}
