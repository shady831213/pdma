//! Shared value declarations and mathematical expressions.
use super::{BitsDefinition, LayoutExpr};
use crate::{BitsFieldDefinition, BitsFieldSpec, EnumVariantSpec, FieldDefinition, RecordSpec};
/// Original encoding definition of an enumeration occurrence.
#[derive(Debug, Clone, Copy)]
pub enum Enumeration {
    /// Byte-addressed scalar enumeration.
    Scalar(FieldDefinition),
    /// Packed bit enumeration.
    Bits(BitsFieldDefinition),
}
impl Enumeration {
    /// Declared alternatives from the original encoding definition.
    pub const fn variants(self) -> &'static [EnumVariantSpec] {
        let definition = match self {
            Self::Scalar(v) => v.enum_definition(),
            Self::Bits(v) => v.enum_definition(),
        };
        match definition {
            Some(v) => v.variants(),
            None => &[],
        }
    }
}
/// A value definition or its contained semantic member.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// Byte-addressed scalar value.
    Scalar(FieldDefinition),
    /// Physical field projected from canonical record storage.
    StorageField(&'a crate::FieldSpec),
    /// Completed member of an opaque storage envelope.
    StorageMember(&'a crate::ResolvedMember),
    /// An explicitly opaque byte envelope usable in either domain.
    Opaque(&'a RecordSpec),
    /// Reusable or inline bits.
    Bits(&'a BitsDefinition),
    /// Inline enumeration.
    Enum(Enumeration),
    /// Contained bit field.
    BitField(&'a BitsFieldSpec),
    /// Enumeration alternative.
    Variant(&'a EnumVariantSpec),
    /// Mathematical layout expression.
    Expression(&'a LayoutExpr),
}
impl Node<'_> {
    /// Iterates immediate semantic children in source order.
    pub fn children(self) -> std::vec::IntoIter<Self> {
        let nodes = match self {
            Self::StorageMember(member) => match member.kind() {
                crate::ResolvedMemberKind::Field { definition, .. } => {
                    vec![Self::field(definition)]
                }
                crate::ResolvedMemberKind::Record { .. } => Vec::new(),
            },
            Self::StorageField(field) => vec![Self::field(field.definition())],
            Self::Scalar(definition) => definition.bits().iter().map(Self::BitField).collect(),
            Self::Bits(bits) => bits.fields().iter().map(Self::BitField).collect(),
            Self::Enum(enumeration) => enumeration.variants().iter().map(Self::Variant).collect(),
            Self::BitField(field) => field
                .definition()
                .enum_definition()
                .map(|_| Self::Enum(Enumeration::Bits(field.definition())))
                .into_iter()
                .collect(),
            Self::Variant(_) | Self::Opaque(_) => Vec::new(),
            Self::Expression(expression) => match expression {
                LayoutExpr::Add(a, b)
                | LayoutExpr::Subtract(a, b)
                | LayoutExpr::Multiply(a, b)
                | LayoutExpr::AlignUp(a, b)
                | LayoutExpr::Maximum(a, b) => vec![Self::Expression(a), Self::Expression(b)],
                LayoutExpr::Constant(_)
                | LayoutExpr::Parameter(_)
                | LayoutExpr::Value(_)
                | LayoutExpr::Unsupported(_) => Vec::new(),
            },
        };
        nodes.into_iter()
    }
    pub(crate) const fn field(definition: FieldDefinition) -> Self {
        match definition.enum_definition() {
            Some(_) => Self::Enum(Enumeration::Scalar(definition)),
            None => Self::Scalar(definition),
        }
    }
}
