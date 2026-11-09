use super::{Context, Node};
use crate::export::{Visit, view};

impl<'a> Context<'a> {
    /// Enclosing placement metadata without referenced target access.
    pub fn placement(&self) -> Option<view::Placement<'a>> {
        core::iter::once(self.0.node())
            .chain(self.0.ancestors().iter().rev().copied())
            .find_map(|node| match node {
                Node::Placement(value) => Some(view::Placement(value)),
                _ => None,
            })
    }
    /// Selects a property of the nearest enclosing placement or map layout.
    pub fn layout(
        &self,
        kind: crate::traversal::register::LayoutPropertyKind,
    ) -> Option<Visit<Self>> {
        self.layout_owner()?.children().find(|visit| matches!(visit.node(), Node::LayoutProperty(property) if property.kind == kind))
            .map(|visit| Visit::new(Self(visit.into_context())))
    }
    /// Constructor argument visits from the nearest enclosing placement.
    pub fn arguments(&self) -> std::vec::IntoIter<Visit<Self>> {
        let Some(owner) = self
            .layout_owner()
            .filter(|ctx| matches!(ctx.node(), Node::Placement(_)))
        else {
            return Vec::new().into_iter();
        };
        owner
            .children()
            .filter(|visit| matches!(visit.node(), Node::Argument(_)))
            .map(|visit| Visit::new(Self(visit.into_context())))
            .collect::<Vec<_>>()
            .into_iter()
    }
    fn layout_owner(&self) -> Option<crate::traversal::register::Context<'a>> {
        let mut owner = self.0.clone();
        while !matches!(owner.node(), Node::Placement(_) | Node::MapLayout(_)) {
            owner = owner.parent()?;
        }
        Some(owner)
    }
}
