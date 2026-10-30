//! Ordered layout formulas shared by checked evaluation and exporters.
mod bind;
mod build;
mod values;
use super::{LayoutExpr, MapDefinition, PlanValue};
use crate::{
    Documentation,
    graph::{DefinitionId, Reference},
};
pub(super) use values::Values as EvaluationValues;

/// Symbolic geometry for one ordered placement.
#[derive(Debug, Clone)]
pub struct PlacementPlan {
    pub(super) resolved_arguments: Vec<usize>,
    /// Placement edge and its declaration name.
    pub reference: Reference,
    /// Resolved child identity without executing a definition callback.
    pub child_definition: DefinitionId,
    /// Resolved child schema name, before target naming.
    pub child_name: &'static str,
    /// Explicit bank identity, including banks of one.
    pub bank: bool,
    /// Placement documentation.
    pub documentation: Documentation,
    /// Validated child geometry source.
    pub child: ChildLayout,
    /// First child offset.
    pub offset: LayoutExpr,
    /// Number of children.
    pub count: LayoutExpr,
    /// Distance between children.
    pub stride: LayoutExpr,
    /// Exclusive occupied end, excluding trailing stride padding.
    pub end: LayoutExpr,
    /// Maximum occupied end after this placement.
    pub cursor: LayoutExpr,
}
/// Geometry of a valid placeable child, prepared once by the common builder.
#[derive(Debug, Clone)]
pub enum ChildLayout {
    /// A fixed register or opaque byte envelope with no constructor arguments.
    Fixed {
        /// Required child placement alignment.
        alignment: usize,
        /// Occupied child extent.
        extent: usize,
    },
    /// A validated child map invocation retaining its parameter bindings.
    Call {
        /// Expressions in the enclosing map's parameter scope.
        arguments: Vec<LayoutExpr>,
        /// Prepared child map whose parameters are bound by this call.
        plan: Box<MapLayoutPlan>,
    },
}
/// An instance-independent map program in declaration order.
#[derive(Debug, Clone)]
pub struct MapLayoutPlan {
    pub(super) parameters: &'static [&'static str],
    pub(super) alignment: usize,
    pub(super) placements: Vec<PlacementPlan>,
    pub(super) extent: LayoutExpr,
}
impl MapLayoutPlan {
    /// Constructor parameters in declaration order.
    pub const fn parameters(&self) -> &'static [&'static str] {
        self.parameters
    }
    /// Required alignment when this map is placed as a child.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }
    /// Ordered child formulas; each field may refer to prior evaluated values.
    pub fn placements(&self) -> &[PlacementPlan] {
        &self.placements
    }
    /// Final extent after all placements.
    pub const fn extent(&self) -> &LayoutExpr {
        &self.extent
    }
}
