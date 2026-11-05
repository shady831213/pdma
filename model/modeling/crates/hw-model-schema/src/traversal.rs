//! Target-neutral, framework-bound traversal of immutable semantic trees.
mod bound;
mod id;
mod schema;
mod visit;
pub use schema::Schema;
/// Source-stage traversal before validation and layout.
pub mod source;
pub use bound::{BoundContext, DomainNode};
pub use id::{Edge, NodeId, SourceRole};
mod source_edge;
pub use visit::{Context, Visit};
/// Record containment and reference expansion.
pub mod record;
/// Register containment, placements and expressions.
pub mod register;
