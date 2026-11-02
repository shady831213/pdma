use super::register::Node;
use super::{MapDefinition, MapPlacement};
use crate::analysis::{BankView, RegisterMapView, RegisterView};
use crate::analysis::{CompleteMapLayout, CompletePlacementLayout};
mod semantic;

/// A register map declaration or a concrete permission projection of one.
#[derive(Debug, Clone, Copy)]
pub enum MapView<'a> {
    /// Original declaration, retaining parameters and references.
    Declared(&'a MapDefinition),
    /// Source declaration with checked mathematical layout properties.
    Analyzed {
        /// Original declaration.
        declaration: &'a MapDefinition,
        /// Checked layout formulas.
        plan: &'a CompleteMapLayout,
        /// Concrete constructor bindings for this root.
        arguments: Option<&'a [usize]>,
    },
    /// Checked concrete layout with effective permissions.
    Resolved(&'a RegisterMapView),
}

/// Location of a register in an analyzed map, independent of target spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterLocation {
    /// A standalone register definition.
    Root,
    /// A standalone or expanded register in a concrete map.
    Standalone(usize),
    /// A repeated block's template member.
    Template {
        /// Bank declaration index.
        bank: usize,
        /// Member declaration index.
        member: usize,
    },
    /// A concrete repeated block member.
    Instance {
        /// Bank declaration index.
        bank: usize,
        /// Instance index.
        instance: usize,
        /// Member declaration index.
        member: usize,
    },
}

/// A declared placement or a checked concrete placement of the same schema.
#[derive(Debug, Clone, Copy)]
pub enum PlacementView<'a> {
    /// A placement in the source declaration.
    Declared(&'a MapPlacement),
    /// Source placement with checked mathematical layout properties.
    Analyzed {
        /// Original source placement.
        declaration: &'a MapPlacement,
        /// Checked properties of the same placement.
        plan: &'a CompletePlacementLayout,
    },
    /// A concrete register placement.
    Register {
        /// Effective register layout and permissions.
        view: &'a RegisterView,
        /// Position within the enclosing analyzed map.
        location: RegisterLocation,
    },
    /// A concrete repeated block.
    Bank {
        /// Block metadata and instances.
        view: &'a BankView,
        /// Bank declaration index.
        index: usize,
        /// Whether registers are already expanded as root children.
        expanded: bool,
    },
}

impl<'a> MapView<'a> {
    /// Iterates members without following their reusable definition references.
    pub fn children(self) -> std::vec::IntoIter<Node<'a>> {
        match self {
            Self::Declared(map) => semantic::map(map, None, None),
            Self::Analyzed {
                declaration,
                plan,
                arguments,
            } => semantic::map(declaration, Some(plan), arguments),
            Self::Resolved(map) => map
                .registers
                .iter()
                .enumerate()
                .map(|(index, view)| {
                    Node::Placement(PlacementView::Register {
                        view,
                        location: RegisterLocation::Standalone(index),
                    })
                })
                .chain(map.banks.iter().enumerate().map(|(index, view)| {
                    Node::Placement(PlacementView::Bank {
                        view,
                        index,
                        expanded: map.expanded_banks,
                    })
                }))
                .collect(),
        }
        .into_iter()
    }
}

impl<'a> PlacementView<'a> {
    /// Iterates concrete contained registers; a declared child remains a reference.
    pub fn children(self) -> std::vec::IntoIter<Node<'a>> {
        let nodes = match self {
            Self::Declared(placement) => semantic::placement(placement, None),
            Self::Analyzed { declaration, plan } => semantic::placement(declaration, Some(plan)),
            Self::Bank { expanded: true, .. } => Vec::new(),
            Self::Register { view, .. } => vec![Node::Register(&view.register)],
            Self::Bank {
                view,
                index,
                expanded: false,
            } => {
                view.registers
                    .iter()
                    .enumerate()
                    .map(|(member, register)| {
                        Node::Placement(Self::Register {
                            view: register,
                            location: RegisterLocation::Template {
                                bank: index,
                                member,
                            },
                        })
                    })
                    .chain(view.instances.iter().enumerate().flat_map(
                        move |(instance, registers)| {
                            registers.iter().enumerate().map(move |(member, register)| {
                                Node::Placement(Self::Register {
                                    view: register,
                                    location: RegisterLocation::Instance {
                                        bank: index,
                                        instance,
                                        member,
                                    },
                                })
                            })
                        },
                    ))
                    .collect()
            }
        };
        nodes.into_iter()
    }
}
