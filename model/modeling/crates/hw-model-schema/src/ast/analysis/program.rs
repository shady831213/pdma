//! Target-independent computation of a register map's layout results.
mod expression;
mod lower;
use crate::ast::DefinitionId;
pub use expression::{Expression, Symbol};

/// Property of one placed register or bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutProperty {
    /// First byte offset.
    Offset,
    /// Number of instances.
    Count,
    /// Distance between instance starts.
    Stride,
}
impl LayoutProperty {
    /// Source-independent spelling of a result property.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Count => "count",
            Self::Stride => "stride",
        }
    }
}

/// A member of the layout function's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Member {
    /// Total occupied extent.
    Extent,
    /// One placement property, preserving its source name.
    Placement {
        /// Source placement name.
        name: &'static str,
        /// Selected property.
        property: LayoutProperty,
    },
}

/// A call to another definition's layout function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Local placement receiving this result.
    pub name: &'static str,
    /// Stable identity used for target-specific alias lookup.
    pub definition: DefinitionId,
    /// Source name used when no target alias is provided.
    pub definition_name: &'static str,
    /// Arguments already resolved in the caller's parameter scope.
    pub arguments: Vec<Expression>,
}

/// One ordered computation; backends only select syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// Evaluate a referenced child layout before using its extent.
    Call(Call),
    /// Define an immutable local intermediate.
    Define {
        /// Symbol introduced here.
        target: Symbol,
        /// Computed initial value.
        value: Expression,
    },
    /// Assign a previously declared local or result member.
    Assign {
        /// Destination symbol.
        target: Symbol,
        /// Computed value.
        value: Expression,
    },
}

/// Complete analyzed layout function, independent of output language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutProgram {
    /// Required alignment of the enclosing map.
    pub alignment: usize,
    /// Parameters in declaration order.
    pub parameters: Vec<&'static str>,
    /// Result structure members in declaration order.
    pub members: Vec<Member>,
    /// Mutable locals declared before computation begins.
    pub locals: Vec<Symbol>,
    /// Checked computation order, including initialization and final extent.
    pub body: Vec<Statement>,
}
