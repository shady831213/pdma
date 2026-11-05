use super::{Edge, SourceRole};
use crate::ast::register::{Attribute, Node};
pub(super) const fn attribute(value: Attribute<'_>) -> SourceRole {
    SourceRole::Attribute(match value {
        Attribute::Width(_) => "width",
        Attribute::Endian(_) => "endian",
        Attribute::Access(_) => "access",
        Attribute::Reset { .. } => "reset",
        Attribute::WritableMask(_) => "writable_mask",
        Attribute::Alignment(_) => "alignment",
        Attribute::ExplicitExtent(_) => "explicit_extent",
        Attribute::ExplicitOffset(_) => "explicit_offset",
        Attribute::Count(_) => "count",
        Attribute::Bank(_) => "bank",
    })
}
pub(super) const fn register(node: Node<'_>, index: usize) -> Edge {
    let role = match node {
        Node::Register(_) | Node::RegisterMap(_) => SourceRole::Declaration,
        Node::RegisterField(_) => SourceRole::Field,
        Node::Placement(_) => SourceRole::Placement,
        Node::Parameter(_) => SourceRole::Parameter,
        Node::Argument(_) => SourceRole::Argument,
        Node::Attribute(value) => attribute(value),
        Node::Value(_) => SourceRole::Value,
        Node::LayoutProperty(_) => return Edge::Derived(index),
    };
    Edge::Source(role, 0)
}
