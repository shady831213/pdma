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
    /// Reusable definition edge owned by a field or case.
    Reference(&'a Reference),
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
                    FieldType::Enum(definition) => vec![Self::Value(value::Node::Enum(
                        value::Enumeration::Scalar(*definition),
                    ))],
                    FieldType::Bits(definition) => vec![Self::Value(value::Node::Bits(definition))],
                    FieldType::Record(record) => vec![Self::Record(record)],
                    FieldType::Opaque(spec) => vec![Self::Value(value::Node::Opaque(spec))],
                    FieldType::Union(union) => vec![Self::Union(union)],
                    FieldType::Reference(reference) => vec![Self::Reference(reference)],
                    FieldType::Scalar {
                        definition,
                        constant,
                    } => constant
                        .as_ref()
                        .map(Self::Constant)
                        .into_iter()
                        .chain([Self::Value(value::Node::Scalar(*definition))])
                        .collect(),
                });
                nodes
            }
            Self::Case(case) => match case.body() {
                CaseBody::Inline(record) => vec![Self::Record(record)],
                CaseBody::Bits(bits) => vec![Self::Value(value::Node::Bits(bits))],
                CaseBody::Union(union) => vec![Self::Union(union)],
                CaseBody::Opaque(spec) => vec![Self::Value(value::Node::Opaque(spec))],
                CaseBody::Reference(reference) => vec![Self::Reference(reference)],
            },
            Self::Value(value) => value
                .children()
                .map(|v| match v {
                    value::Node::Reference(r) => Self::Reference(r),
                    other => Self::Value(other),
                })
                .collect(),
            Self::Attribute(_) | Self::Constant(_) | Self::Reference(_) => Vec::new(),
        };
        nodes.into_iter()
    }
    /// Reference owned directly by this node, excluding descendants.
    pub const fn reference(self) -> Option<&'a Reference> {
        match self {
            Self::Reference(reference) | Self::Value(value::Node::Reference(reference)) => {
                Some(reference)
            }
            Self::Field(_)
            | Self::Case(_)
            | Self::Record(_)
            | Self::Union(_)
            | Self::Attribute(_)
            | Self::Constant(_)
            | Self::Value(_) => None,
        }
    }
}
