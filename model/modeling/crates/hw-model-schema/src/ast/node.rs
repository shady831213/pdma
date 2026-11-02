use super::{DefinitionBody, MapView, Reference, record, register, value};
/// Source discovery sum; domain backends receive their independent node family.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// Record-domain source.
    Record(record::Node<'a>),
    /// Register-domain source.
    Register(register::Node<'a>),
    /// Reusable value source.
    Value(value::Node<'a>),
}
impl<'a> Node<'a> {
    /// Iterates source containment without following definition references.
    pub fn children(self) -> std::vec::IntoIter<Self> {
        match self {
            Self::Record(node) => node
                .children()
                .map(|n| match n {
                    record::Node::Value(value) => Self::Value(value),
                    record::Node::Record(_)
                    | record::Node::Union(_)
                    | record::Node::Field(_)
                    | record::Node::Case(_)
                    | record::Node::Attribute(_)
                    | record::Node::Constant(_) => Self::Record(n),
                })
                .collect::<Vec<_>>(),
            Self::Register(node) => node
                .children()
                .map(|n| match n {
                    register::Node::Value(value) => Self::Value(value),
                    register::Node::Register(_)
                    | register::Node::RegisterMap(_)
                    | register::Node::Placement(_)
                    | register::Node::RegisterField(_)
                    | register::Node::Parameter(_)
                    | register::Node::Argument(_)
                    | register::Node::Attribute(_)
                    | register::Node::LayoutProperty(_) => Self::Register(n),
                })
                .collect(),
            Self::Value(node) => node.children().map(Self::Value).collect(),
        }
        .into_iter()
    }
    /// Iterates every reference edge, retaining duplicate uses.
    pub fn references(self) -> std::vec::IntoIter<&'a Reference> {
        let direct = match self {
            Self::Record(node) => node.reference(),
            Self::Register(node) => node.reference(),
            Self::Value(_) => None,
        };
        direct
            .into_iter()
            .chain(self.children().flat_map(Self::references))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
impl DefinitionBody {
    /// Borrows a declaration through its source domain.
    pub const fn node(&self) -> Node<'_> {
        match self {
            Self::Record(v) => Node::Record(record::Node::Record(v)),
            Self::Union(v) => Node::Record(record::Node::Union(v)),
            Self::Opaque(v) => Node::Value(value::Node::Opaque(v)),
            Self::Register(v) => Node::Register(register::Node::Register(v)),
            Self::RegisterMap(v) => {
                Node::Register(register::Node::RegisterMap(MapView::Declared(v)))
            }
            Self::Bits(v) => Node::Value(value::Node::Bits(v)),
        }
    }
}
