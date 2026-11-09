//! Symbolic register-map placement rules and checked instance evaluation.
mod expression;
mod plan;
mod resolve;
mod walk;
use super::{Definition, Reference, ReferenceKind};
use crate::Documentation;
pub use expression::{ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, PlanValue};
pub use plan::{ChildLayout, MapLayoutPlan, PlacementPlan};
pub use resolve::{MapGeometry, PlacementGeometry};
pub use walk::{MapPathSegment, RegisterInstance};

/// The source relationship at a child map/register placement.
#[derive(Debug, Clone)]
pub enum PlacementBody {
    /// A reusable named definition.
    Reference(Reference),
    /// Owned child declaration retaining its independent parameter scope.
    Inline(Box<Definition>),
}
impl PlacementBody {
    /// Obtains the child without traversing its own dependencies.
    pub fn target(&self) -> Definition {
        match self {
            Self::Reference(reference) => reference.target(),
            Self::Inline(child) => child.as_ref().clone(),
        }
    }
}

/// One ordered placement, retaining its child type and parameter bindings.
#[derive(Debug, Clone)]
pub struct MapPlacement {
    name: &'static str,
    body: PlacementBody,
    arguments: Vec<LayoutExpr>,
    count: LayoutExpr,
    offset: Option<usize>,
    bank: bool,
    documentation: Documentation,
}
impl MapPlacement {
    /// Creates a child placement with explicit constructor arguments and count.
    pub fn new(
        name: &'static str,
        target: fn() -> Definition,
        arguments: Vec<LayoutExpr>,
        count: LayoutExpr,
        offset: Option<usize>,
        bank: bool,
    ) -> Self {
        Self {
            name,
            body: PlacementBody::Reference(Reference::new(name, ReferenceKind::Placement, target)),
            arguments,
            count,
            offset,
            bank,
            documentation: Documentation::EMPTY,
        }
    }
    /// Attaches placement documentation, independently of the referenced type.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }
    /// Member name, independent of containment or reference representation.
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Explicit child containment or reusable reference.
    pub const fn body(&self) -> &PlacementBody {
        &self.body
    }
    /// Obtains the exact child declaration in either representation.
    pub fn target(&self) -> Definition {
        self.body.target()
    }
    pub(crate) fn with_body(&self, body: PlacementBody) -> Self {
        Self {
            body,
            ..self.clone()
        }
    }
    /// Constructor arguments in the child's declared parameter order.
    pub fn arguments(&self) -> &[LayoutExpr] {
        &self.arguments
    }
    /// Number of placed instances; one does not erase explicit bank identity.
    pub const fn count(&self) -> &LayoutExpr {
        &self.count
    }
    /// Explicit offset override; absent means aligned sequential placement.
    pub const fn offset(&self) -> Option<usize> {
        self.offset
    }
    /// Whether the declaration explicitly used a bank.
    pub const fn is_bank(&self) -> bool {
        self.bank
    }
    /// Placement documentation.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }
}

/// A map definition retains symbolic placement rules independently of a resolved address table.
#[derive(Debug, Clone)]
pub struct MapDefinition {
    parameters: &'static [&'static str],
    alignment: usize,
    extent: Option<usize>,
    placements: Vec<MapPlacement>,
}
impl MapDefinition {
    /// Describes ordered, noninterleaved placements with optional explicit extent.
    pub const fn new(
        parameters: &'static [&'static str],
        alignment: usize,
        extent: Option<usize>,
        placements: Vec<MapPlacement>,
    ) -> Self {
        Self {
            parameters,
            alignment,
            extent,
            placements,
        }
    }
    /// Declared instance parameter names in constructor order.
    pub const fn parameters(&self) -> &'static [&'static str] {
        self.parameters
    }
    /// Required alignment of this map when placed as a child.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }
    /// Explicit extent override; absent means the maximum occupied end.
    pub const fn explicit_extent(&self) -> Option<usize> {
        self.extent
    }
    /// Declared placements; holes are represented by alignment and offset rules.
    pub fn placements(&self) -> &[MapPlacement] {
        &self.placements
    }
}
