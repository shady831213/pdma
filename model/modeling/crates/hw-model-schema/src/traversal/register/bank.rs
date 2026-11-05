use super::Node;
use crate::ast::register::{LayoutProperty, LayoutPropertyKind as K};
use crate::{analysis::BankView, ast::RegisterLocation};
/// A completed bank; topology selection belongs to traversal.
#[derive(Debug, Clone, Copy)]
pub struct Bank<'a> {
    pub(super) view: &'a BankView,
    pub(super) index: usize,
    pub(super) expanded: bool,
}
impl<'a> Bank<'a> {
    /// Stable bank position in the selected map.
    pub const fn index(self) -> usize {
        self.index
    }
    /// Original placement name.
    pub const fn name(self) -> &'static str {
        self.view.name
    }
    /// Source path of this bank.
    pub fn path(self) -> &'a str {
        &self.view.path
    }
    /// Structural path excluding the selected map root.
    pub fn segments(self) -> &'a [crate::analysis::ViewPathSegment] {
        &self.view.segments
    }
    /// Placement documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.view.documentation
    }
    pub(super) fn children(self) -> Vec<Node<'a>> {
        let mut nodes = Vec::new();
        if self.expanded {
            nodes.push(Node::BankProperty(BankProperty::Index(self.index)));
            if let Some((parent, instance)) = self.view.parent {
                nodes.push(Node::BankProperty(BankProperty::ParentIndex(parent)));
                nodes.push(Node::BankProperty(BankProperty::ParentInstance(instance)));
            }
        }
        nodes.extend(
            [
                (K::Offset, &self.view.base),
                (K::Count, &self.view.count),
                (K::Stride, &self.view.stride),
            ]
            .into_iter()
            .map(|(kind, expression)| Node::LayoutProperty(LayoutProperty { kind, expression })),
        );
        if !self.expanded {
            nodes.extend(
                self.view
                    .registers
                    .iter()
                    .enumerate()
                    .map(|(member, view)| Node::RegisterPlacement {
                        view,
                        location: RegisterLocation::Template {
                            bank: self.index,
                            member,
                        },
                    }),
            );
            nodes.extend(
                self.view
                    .instances
                    .iter()
                    .enumerate()
                    .flat_map(|(instance, rs)| {
                        rs.iter()
                            .enumerate()
                            .map(move |(member, view)| Node::RegisterPlacement {
                                view,
                                location: RegisterLocation::Instance {
                                    bank: self.index,
                                    instance,
                                    member,
                                },
                            })
                    }),
            );
        }
        nodes
    }
}
/// Final scalar bank properties selected by semantic traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankProperty {
    /// Stable bank identity.
    Index(usize),
    /// Enclosing bank identity.
    ParentIndex(usize),
    /// Enclosing bank instance.
    ParentInstance(usize),
}
