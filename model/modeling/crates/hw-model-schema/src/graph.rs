//! Definition discovery and dependency traversal, independent of output format.
//!
//! Select roots with [`SchemaSet`](crate::graph::SchemaSet); resolution retains both unique definitions and
//! every reference edge. No registry is required for manually selected roots.
mod bits;
mod definition;
#[cfg(feature = "discovery")]
mod discovery;
mod instance;
mod map;
mod register;
mod set;
#[cfg(feature = "discovery")]
pub use discovery::{Registration, SchemaRegistry};

pub use bits::BitsDefinition;
pub use definition::{Definition, DefinitionBody, DefinitionId, Reference, ReferenceKind};
pub use instance::MapInstance;
pub use map::{
    LayoutError, LayoutExpr, MapDefinition, MapGeometry, MapPlacement, PlacementGeometry,
};
pub use register::RegisterInfo;
pub use set::{GraphError, SchemaGraph, SchemaSet};

/// Metadata entry point implemented by generated schema types.
pub trait DescribeSchema {
    /// Returns the definition without recursively constructing its dependencies.
    fn schema_definition() -> Definition;
}
