//! Definition discovery and dependency traversal, independent of output format.
//!
//! Select roots with [`SchemaSet`](crate::ast::SchemaSet); resolution retains both unique definitions and
//! every reference edge. No registry is required for manually selected roots.
mod bits;
mod node;
/// Record declarations and their containment nodes.
pub mod record;
mod view;
pub use node::Node;
pub use record::{Case, CaseBody, Constant, Field, FieldType, RecordDefinition, UnionDefinition};
pub use view::{MapView, PlacementView, RegisterLocation};
mod definition;
#[cfg(feature = "discovery")]
mod discovery;
mod instance;
mod map;
/// Register declarations and their containment nodes.
pub mod register;
mod set;
/// Values shared by record and register schemas.
pub mod value;
#[cfg(feature = "discovery")]
pub use discovery::{Registration, SchemaRegistry};

pub use bits::BitsDefinition;
pub use definition::{Definition, DefinitionBody, DefinitionId, Reference, ReferenceKind};
pub use instance::MapInstance;
pub use map::{
    ChildLayout, ExpressionOp, ExpressionVisitor, LayoutError, LayoutExpr, MapDefinition,
    MapGeometry, MapLayoutPlan, MapPathSegment, MapPlacement, PlacementGeometry, PlacementPlan,
    PlanValue, RegisterInstance,
};
pub use register::RegisterInfo;
pub use set::{GraphError, SchemaGraph, SchemaSet};

/// Metadata entry point implemented by generated schema types.
pub trait DescribeSchema {
    /// Returns the definition without recursively constructing its dependencies.
    fn schema_definition() -> Definition;
}

/// Metadata entry point for source union declarations, including inline unions.
pub trait DescribeUnion {
    /// Constructs the source union without requiring a synthetic dependency edge.
    fn schema_union() -> UnionDefinition;
}
