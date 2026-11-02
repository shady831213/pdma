use super::{
    Attribute, Case, CaseBody, Constant, Field, FieldType, RecordDefinition, UnionDefinition,
};
use crate::ast::{Reference, value};
/// Containment within the record domain.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// Record declaration.
    Record(&'a RecordDefinition),
    /// Union declaration.
    Union(&'a UnionDefinition),
    /// Logical field.
    Field(&'a Field),
    /// Union alternative.
    Case(&'a Case),
    /// Source geometry or encoding attribute.
    Attribute(Attribute),
    /// Declared scalar constant constraint.
    Constant(&'a Constant),
    /// Shared value declaration.
    Value(value::Node<'a>),
}
impl<'a> Node<'a> {
    /// Iterates containment without following reusable references.
    pub fn children(self) -> std::vec::IntoIter<Self> {
        let nodes = match self {
            Self::Record(record) => [
                Self::Attribute(Attribute::Size(record.spec().size())),
                Self::Attribute(Attribute::Endian(record.endian())),
            ]
            .into_iter()
            .chain(
                record
                    .declared_alignment()
                    .map(|a| Self::Attribute(Attribute::Alignment(a))),
            )
            .chain(record.fields().iter().map(Self::Field))
            .collect(),
            Self::Union(union) => union.cases().iter().map(Self::Case).collect(),
            Self::Field(field) => {
                let mut nodes = vec![
                    Self::Attribute(Attribute::Offset(field.offset())),
                    Self::Attribute(Attribute::Size(field.width())),
                    Self::Attribute(Attribute::Endian(field.endian())),
                ];
                nodes.extend(match field.ty() {
                    FieldType::Enum(definition) => definition
                        .enum_definition()
                        .map(value::Node::Enum)
                        .map(Self::Value)
                        .into_iter()
                        .collect(),
                    FieldType::Bits(definition) => vec![Self::Value(value::Node::Bits(definition))],
                    FieldType::Union(union) => vec![Self::Union(union)],
                    FieldType::Reference(_) => Vec::new(),
                    FieldType::Scalar { constant, .. } => {
                        constant.as_ref().map(Self::Constant).into_iter().collect()
                    }
                });
                nodes
            }
            Self::Case(case) => match case.body() {
                CaseBody::Inline(record) => record.fields().iter().map(Self::Field).collect(),
                CaseBody::Reference(_) => Vec::new(),
            },
            Self::Value(value) => value.children().map(Self::Value).collect(),
            Self::Attribute(_) | Self::Constant(_) => Vec::new(),
        };
        nodes.into_iter()
    }
    /// Reference owned directly by this node, excluding descendants.
    pub const fn reference(self) -> Option<&'a Reference> {
        match self {
            Self::Field(field) => match field.ty() {
                FieldType::Reference(reference) => Some(reference),
                FieldType::Enum(_)
                | FieldType::Bits(_)
                | FieldType::Union(_)
                | FieldType::Scalar { .. } => None,
            },
            Self::Case(case) => match case.body() {
                CaseBody::Reference(reference) => Some(reference),
                CaseBody::Inline(_) => None,
            },
            Self::Record(_)
            | Self::Union(_)
            | Self::Attribute(_)
            | Self::Constant(_)
            | Self::Value(_) => None,
        }
    }
}
