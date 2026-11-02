use crate::ast::LayoutExpr;
/// A declared instance parameter, optionally bound for this analyzed root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    /// Source identifier.
    pub name: &'static str,
    /// Constructor position.
    pub index: usize,
    /// Concrete root argument, when selected.
    pub binding: Option<usize>,
}
/// A child constructor argument in the enclosing map's parameter scope.
#[derive(Debug, Clone, Copy)]
pub struct Argument<'a> {
    /// Child parameter name, if the referenced declaration supplies one.
    pub parameter: Option<&'static str>,
    /// Constructor position.
    pub index: usize,
    /// Source argument expression.
    pub expression: &'a LayoutExpr,
}
/// Explicit source map and placement attributes.
#[derive(Debug, Clone, Copy)]
pub enum Attribute<'a> {
    /// Encoded register width in bytes.
    Width(usize),
    /// Register byte order.
    Endian(crate::Endian),
    /// Bus access permission.
    Access(crate::Access),
    /// Reset classification and declared wire value.
    Reset {
        /// Reset owner/classification.
        kind: crate::ResetKind,
        /// Reset wire value.
        value: u64,
    },
    /// Writable register bits.
    WritableMask(u64),
    /// Required map or register alignment.
    Alignment(usize),
    /// Explicit enclosing extent.
    ExplicitExtent(usize),
    /// Explicit placement offset.
    ExplicitOffset(usize),
    /// Declared repetition count.
    Count(&'a LayoutExpr),
    /// Explicit bank identity, even when the count is one.
    Bank(bool),
}
/// Mathematical property derived from placement rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPropertyKind {
    /// First byte offset.
    Offset,
    /// Instance count.
    Count,
    /// Distance between starts.
    Stride,
    /// Exclusive occupied end.
    End,
    /// Maximum occupied end through this placement.
    OccupiedEnd,
    /// Map extent.
    Extent,
}
impl LayoutPropertyKind {
    /// Semantic spelling without a target identifier convention.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Count => "count",
            Self::Stride => "stride",
            Self::End => "end",
            Self::OccupiedEnd => "occupied_end",
            Self::Extent => "extent",
        }
    }
}
/// A checked mathematical layout formula, never a generated statement.
#[derive(Debug, Clone, Copy)]
pub struct LayoutProperty<'a> {
    /// Property described by the formula.
    pub kind: LayoutPropertyKind,
    /// Formula in the enclosing placement/map scope.
    pub expression: &'a LayoutExpr,
}
