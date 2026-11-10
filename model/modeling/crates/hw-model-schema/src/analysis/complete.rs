//! Closed mathematical geometry; child calls and ordered dependencies are eliminated.
mod evaluate;
mod substitute;
mod substitution;
use crate::{
    Documentation,
    ast::{LayoutExpr, MapPlacement},
};

/// Completed geometry for an ordered placement, expressed in its root parameter scope.
#[derive(Debug, Clone)]
pub struct CompletePlacementLayout {
    /// Original reference and member name.
    pub source: MapPlacement,
    /// Completed referenced map with use-site arguments substituted; independent of bank expansion.
    pub referent_layout: Option<Box<CompleteMapLayout>>,
    /// Referenced definition identity.
    pub child_definition: crate::traversal::NodeId,
    /// Source child name.
    pub child_name: String,
    /// Explicit bank identity.
    pub bank: bool,
    /// Placement documentation.
    pub documentation: Documentation,
    /// Child constructor expressions in the root scope.
    pub arguments: Vec<LayoutExpr>,
    /// Names of the referenced child constructor parameters.
    pub child_parameters: &'static [&'static str],
    /// Required child alignment.
    pub child_alignment: usize,
    /// Child extent in the root scope.
    pub child_extent: LayoutExpr,
    /// First instance offset.
    pub offset: LayoutExpr,
    /// Instance count.
    pub count: LayoutExpr,
    /// Instance spacing.
    pub stride: LayoutExpr,
    /// Exclusive occupied end.
    pub end: LayoutExpr,
    /// Maximum occupied end through this placement.
    pub cursor: LayoutExpr,
}
/// Completed mathematical map layout with no unresolved child or placement values.
#[derive(Debug, Clone)]
pub struct CompleteMapLayout {
    pub(super) parameters: &'static [&'static str],
    pub(super) source_parameters: &'static [&'static str],
    pub(super) bindings: Option<Vec<usize>>,
    pub(super) alignment: usize,
    pub(super) placements: Vec<CompletePlacementLayout>,
    pub(super) extent: LayoutExpr,
    pub(super) constraints: Vec<Constraint>,
}
/// Parameter-dependent obligations retained by completed geometry.
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Expression must be nonzero.
    Positive(LayoutExpr),
    /// Offset must satisfy the required power-of-two alignment.
    Aligned(LayoutExpr, usize),
    /// Occupied end must not exceed the available extent.
    Fits(LayoutExpr, LayoutExpr),
    /// Two half-open occupied envelopes must not overlap.
    Disjoint(LayoutExpr, LayoutExpr, LayoutExpr, LayoutExpr),
}
impl CompleteMapLayout {
    /// All geometry obligations, including substituted nested child constraints.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
    /// Original constructor parameters, including bound roots.
    pub const fn source_parameters(&self) -> &'static [&'static str] {
        self.source_parameters
    }
    /// Supplied root bindings, if this is a specialized instance.
    pub fn bindings(&self) -> Option<&[usize]> {
        self.bindings.as_deref()
    }
    /// Unbound root parameters.
    pub const fn parameters(&self) -> &'static [&'static str] {
        self.parameters
    }
    /// Required alignment.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }
    /// Completed placement geometry in source order.
    pub fn placements(&self) -> &[CompletePlacementLayout] {
        &self.placements
    }
    /// Completed extent expression.
    pub const fn extent(&self) -> &LayoutExpr {
        &self.extent
    }
}
