//! Symbolic register-map placement rules and checked instance evaluation.
mod expression;
mod resolve;
use super::{Definition, Reference, ReferenceKind};
use crate::Documentation;
pub use expression::{LayoutError, LayoutExpr};
pub use resolve::{MapGeometry, PlacementGeometry};

/// One ordered placement, retaining its child type and parameter bindings.
#[derive(Debug, Clone)]
pub struct MapPlacement {
    reference: Reference,
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
            reference: Reference::new(name, ReferenceKind::Placement, target),
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
    /// Returns the child-definition reference.
    pub const fn reference(&self) -> &Reference {
        &self.reference
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

/// A map definition retains the layout algorithm, not just one resolved address table.
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
