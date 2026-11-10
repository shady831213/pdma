use super::Context;
use crate::{
    ast::{record::Node, value},
    export::{Visit, view},
    traversal::Edge,
};

impl<'a> Context<'a> {
    /// Bound physical storage fields covered by the current tagged union.
    pub fn union_storage(&self) -> std::vec::IntoIter<Visit<Self>> {
        if !matches!(self.0.node(), Node::Union(_)) {
            return Vec::new().into_iter();
        }
        let Some(parent) = self
            .0
            .parent()
            .filter(|ctx| ctx.node().reference().is_some())
        else {
            return Vec::new().into_iter();
        };
        self.0
            .analysis()
            .union_storage(&parent.local_id())
            .unwrap_or(&[])
            .iter()
            .map(|(index, member)| {
                Visit::new(Self(self.0.descendant(
                    Node::Value(value::Node::StorageField(member)),
                    Edge::Derived(*index),
                )))
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
    /// Enclosing physical storage field, without its value encoding.
    pub fn storage_field(&self) -> Option<view::StorageField<'a>> {
        core::iter::once(self.0.node())
            .chain(self.0.ancestors().iter().rev().copied())
            .find_map(|node| match node {
                Node::Value(value::Node::StorageField(field)) => Some(view::StorageField(field)),
                _ => None,
            })
    }
}
