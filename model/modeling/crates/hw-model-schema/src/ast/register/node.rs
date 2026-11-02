use super::{Argument, Attribute, LayoutProperty, Parameter, RegisterInfo};
use crate::{
    BitsFieldSpec,
    ast::{MapView, PlacementView, Reference, value},
};
/// Containment within the register domain.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// Register declaration.
    Register(&'a RegisterInfo),
    /// Source map with optional checked layout attributes.
    RegisterMap(MapView<'a>),
    /// Register bit field with erased application index.
    RegisterField(&'a BitsFieldSpec<usize>),
    /// Source or concrete placement.
    Placement(PlacementView<'a>),
    /// Map constructor parameter.
    Parameter(Parameter),
    /// Child constructor argument binding.
    Argument(Argument<'a>),
    /// Explicit source attribute.
    Attribute(Attribute<'a>),
    /// Derived layout formula.
    LayoutProperty(LayoutProperty<'a>),
    /// Shared value declaration or expression.
    Value(value::Node<'a>),
}
impl<'a> Node<'a> {
    /// Iterates source containment without following references.
    pub fn children(self) -> std::vec::IntoIter<Self> {
        let nodes = match self {
            Self::Register(register) => {
                let mut nodes = vec![
                    Self::Attribute(Attribute::Width(register.width())),
                    Self::Attribute(Attribute::Alignment(register.alignment())),
                    Self::Attribute(Attribute::Endian(register.endian())),
                    Self::Attribute(Attribute::Access(register.access())),
                    Self::Attribute(Attribute::Reset {
                        kind: register.reset_kind(),
                        value: register.reset_value(),
                    }),
                    Self::Attribute(Attribute::WritableMask(register.writable_mask())),
                ];
                if register.source().is_none() {
                    nodes.extend(register.fields().iter().map(Self::RegisterField));
                }
                nodes
            }
            Self::RegisterMap(map) => map.children().collect(),
            Self::RegisterField(field) => field
                .definition()
                .enum_definition()
                .map(value::Node::Enum)
                .map(Self::Value)
                .into_iter()
                .collect(),
            Self::Placement(placement) => placement.children().collect(),
            Self::Argument(argument) => {
                vec![Self::Value(value::Node::Expression(argument.expression))]
            }
            Self::Attribute(Attribute::Count(expression))
            | Self::LayoutProperty(LayoutProperty { expression, .. }) => {
                vec![Self::Value(value::Node::Expression(expression))]
            }
            Self::Value(value) => value.children().map(Self::Value).collect(),
            Self::Parameter(_)
            | Self::Attribute(
                Attribute::Width(_)
                | Attribute::Endian(_)
                | Attribute::Access(_)
                | Attribute::Reset { .. }
                | Attribute::WritableMask(_)
                | Attribute::Alignment(_)
                | Attribute::ExplicitExtent(_)
                | Attribute::ExplicitOffset(_)
                | Attribute::Bank(_),
            ) => Vec::new(),
        };
        nodes.into_iter()
    }
    /// Reusable definition referenced directly by this node.
    pub const fn reference(self) -> Option<&'a Reference> {
        match self {
            Self::Register(register) => register.source(),
            Self::Placement(
                PlacementView::Declared(p) | PlacementView::Analyzed { declaration: p, .. },
            ) => Some(p.reference()),
            Self::RegisterMap(_)
            | Self::RegisterField(_)
            | Self::Placement(PlacementView::Register { .. } | PlacementView::Bank { .. })
            | Self::Parameter(_)
            | Self::Argument(_)
            | Self::Attribute(_)
            | Self::LayoutProperty(_)
            | Self::Value(_) => None,
        }
    }
}
