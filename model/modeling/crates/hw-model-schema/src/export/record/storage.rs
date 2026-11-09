use super::Context;
use crate::{
    ast::{record::Node, value},
    export::{Visit, view},
    traversal::Edge,
};

impl<'a> Context<'a> {
    /// Bound physical storage fields covered by the current tagged union.
    pub fn union_storage(&self) -> std::vec::IntoIter<Visit<Self>> {
        let Node::Union(union) = self.0.node() else {
            return Vec::new().into_iter();
        };
        if union.selector().is_none() {
            return Vec::new().into_iter();
        }
        let Some(field) = self.0.field() else {
            return Vec::new().into_iter();
        };
        let Some(record) = self.0.ancestors().iter().rev().find_map(|node| match node {
            Node::Record(record) => Some(record),
            _ => None,
        }) else {
            return Vec::new().into_iter();
        };
        record
            .spec()
            .fields()
            .iter()
            .enumerate()
            .filter(|(_, member)| {
                member.offset() >= field.offset()
                    && member.offset().saturating_sub(field.offset()) < field.width()
            })
            .map(|(index, member)| {
                Visit::new(Self(self.0.descendant(
                    Node::Value(value::Node::StorageField(member)),
                    Edge::Derived(index),
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
