use crate::{
    BitsFieldSpec,
    analysis::{
        CompleteMapLayout, CompletePlacementLayout, ConcretePlacement, Layout, RegisterMapView,
        Unit,
    },
    ast::{
        RegisterInfo, RegisterLocation,
        register::{Argument, Attribute, LayoutProperty, LayoutPropertyKind, Parameter},
        value,
    },
};
/// Borrowed traversal of completed register semantics, without preparation states.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// A completed selected root.
    Root(&'a Unit),
    /// Completed map layout (symbolic or constant).
    MapLayout(&'a CompleteMapLayout),
    /// Concrete permission projection.
    RegisterMap(&'a RegisterMapView),
    /// Register declaration.
    Register(&'a RegisterInfo),
    /// Register field.
    RegisterField(&'a BitsFieldSpec<usize>),
    /// Completed placement layout.
    Placement(&'a CompletePlacementLayout),
    /// Concrete member or bank.
    ConcretePlacement(ConcretePlacement<'a>),
    /// Root parameter.
    Parameter(Parameter),
    /// Child binding.
    Argument(Argument<'a>),
    /// Source attribute.
    Attribute(Attribute<'a>),
    /// Completed layout expression.
    LayoutProperty(LayoutProperty<'a>),
    /// Shared semantic value.
    Value(value::Node<'a>),
}
impl<'a> Node<'a> {
    /// Iterates complete semantic containment in source order.
    pub fn children(self) -> std::vec::IntoIter<Self> {
        let nodes = match self {
            Self::Root(unit) => Self::root_children(unit),
            Self::MapLayout(_) | Self::Placement(_) => self.layout_children(),
            Self::RegisterMap(map) => map
                .registers
                .iter()
                .enumerate()
                .map(|(index, view)| {
                    Self::ConcretePlacement(ConcretePlacement::Register {
                        view,
                        location: RegisterLocation::Standalone(index),
                    })
                })
                .chain(map.banks.iter().enumerate().map(|(index, view)| {
                    Self::ConcretePlacement(ConcretePlacement::Bank {
                        view,
                        index,
                        expanded: map.expanded_banks,
                    })
                }))
                .collect(),
            Self::ConcretePlacement(ConcretePlacement::Register { view, .. }) => {
                vec![Self::Register(&view.register)]
            }
            Self::ConcretePlacement(ConcretePlacement::Bank {
                view,
                index,
                expanded: false,
            }) => view
                .registers
                .iter()
                .enumerate()
                .map(|(member, view)| {
                    Self::ConcretePlacement(ConcretePlacement::Register {
                        view,
                        location: RegisterLocation::Template {
                            bank: index,
                            member,
                        },
                    })
                })
                .chain(
                    view.instances
                        .iter()
                        .enumerate()
                        .flat_map(move |(instance, rs)| {
                            rs.iter().enumerate().map(move |(member, view)| {
                                Self::ConcretePlacement(ConcretePlacement::Register {
                                    view,
                                    location: RegisterLocation::Instance {
                                        bank: index,
                                        instance,
                                        member,
                                    },
                                })
                            })
                        }),
                )
                .collect(),
            Self::Register(v) => crate::ast::register::Node::Register(v)
                .children()
                .filter_map(Self::source)
                .chain(
                    v.source()
                        .into_iter()
                        .flat_map(|_| v.fields().iter().map(Self::RegisterField)),
                )
                .collect(),
            Self::RegisterField(v) => crate::ast::register::Node::RegisterField(v)
                .children()
                .filter_map(Self::source)
                .collect(),
            Self::Argument(v) => vec![Self::Value(value::Node::Expression(v.expression))],
            Self::LayoutProperty(v) => vec![Self::Value(value::Node::Expression(v.expression))],
            Self::Attribute(Attribute::Count(e)) => vec![Self::Value(value::Node::Expression(e))],
            Self::Value(v) => v.children().map(Self::Value).collect(),
            Self::ConcretePlacement(ConcretePlacement::Bank { expanded: true, .. })
            | Self::Parameter(_)
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
    fn root_children(unit: &'a Unit) -> Vec<Self> {
        match &unit.body {
            Layout::Map(map) => map
                .plan
                .as_ref()
                .map(|map| Self::MapLayout(map))
                .into_iter()
                .chain(map.instance.as_ref().map(Self::RegisterMap))
                .collect(),
            Layout::None | Layout::Record(_) => unit
                .declaration
                .as_ref()
                .into_iter()
                .filter_map(|d| match d {
                    crate::ast::DefinitionBody::Register(v) => Some(Self::Register(v)),
                    crate::ast::DefinitionBody::Bits(v) => Some(Self::Value(value::Node::Bits(v))),
                    crate::ast::DefinitionBody::Opaque(v) => {
                        Some(Self::Value(value::Node::Opaque(v)))
                    }
                    crate::ast::DefinitionBody::RegisterMap(_)
                    | crate::ast::DefinitionBody::Record(_)
                    | crate::ast::DefinitionBody::Union(_) => None,
                })
                .collect(),
        }
    }
    fn layout_children(self) -> Vec<Self> {
        match self {
            Self::MapLayout(map) => map
                .source_parameters()
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    Self::Parameter(Parameter {
                        name,
                        index,
                        binding: map.bindings().and_then(|b| b.get(index).copied()),
                    })
                })
                .chain(core::iter::once(Self::Attribute(Attribute::Alignment(
                    map.alignment(),
                ))))
                .chain(map.placements().iter().map(Self::Placement))
                .chain(core::iter::once(Self::LayoutProperty(LayoutProperty {
                    kind: LayoutPropertyKind::Extent,
                    expression: map.extent(),
                })))
                .collect(),
            Self::Placement(p) => p
                .arguments
                .iter()
                .enumerate()
                .map(|(index, expression)| {
                    Self::Argument(Argument {
                        parameter: p.child_parameters.get(index).copied(),
                        index,
                        expression,
                    })
                })
                .chain([Self::Attribute(Attribute::Bank(p.bank))])
                .chain(
                    [
                        (LayoutPropertyKind::Offset, &p.offset),
                        (LayoutPropertyKind::Count, &p.count),
                        (LayoutPropertyKind::Stride, &p.stride),
                        (LayoutPropertyKind::End, &p.end),
                        (LayoutPropertyKind::OccupiedEnd, &p.cursor),
                    ]
                    .map(|(kind, expression)| {
                        Self::LayoutProperty(LayoutProperty { kind, expression })
                    }),
                )
                .collect(),
            Self::Root(_)
            | Self::RegisterMap(_)
            | Self::Register(_)
            | Self::RegisterField(_)
            | Self::ConcretePlacement(_)
            | Self::Parameter(_)
            | Self::Argument(_)
            | Self::Attribute(_)
            | Self::LayoutProperty(_)
            | Self::Value(_) => Vec::new(),
        }
    }
    const fn source(node: crate::ast::register::Node<'a>) -> Option<Self> {
        use crate::ast::register::Node as N;
        match node {
            N::RegisterField(v) => Some(Self::RegisterField(v)),
            N::Attribute(v) => Some(Self::Attribute(v)),
            N::Value(v) => Some(Self::Value(v)),
            N::Register(_)
            | N::RegisterMap(_)
            | N::Placement(_)
            | N::Parameter(_)
            | N::Argument(_)
            | N::LayoutProperty(_) => None,
        }
    }
}
