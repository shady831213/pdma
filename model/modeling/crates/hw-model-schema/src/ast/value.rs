//! Shared value declarations and mathematical expressions.
use super::{BitsDefinition, LayoutExpr};
use crate::{BitsFieldSpec, EnumDefinition, EnumVariantSpec, RecordSpec};
/// A value definition or its contained semantic member.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// An explicitly opaque byte envelope usable in either domain.
    Opaque(&'a RecordSpec),
    /// Reusable or inline bits.
    Bits(&'a BitsDefinition),
    /// Inline enumeration.
    Enum(EnumDefinition),
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
            Self::Bits(bits) => bits.fields().iter().map(Self::BitField).collect(),
            Self::Enum(enumeration) => enumeration.variants().iter().map(Self::Variant).collect(),
            Self::BitField(field) => field
                .definition()
                .enum_definition()
                .map(Self::Enum)
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
}
