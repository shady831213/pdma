//! Format-neutral, owned artifact export vocabulary.

use core::{convert::Infallible, fmt};
mod symbols;
pub use symbols::{SymbolCollision, SymbolScope};
mod schema;
mod visit;
mod walk;
pub use schema::{
    BankView, Bindings, ExportId, ExportPathSegment, PreparationError, PreparedBody,
    PreparedDefinition, PreparedMap, PreparedReference, PreparedSchema, RegisterMapView,
    RegisterView, SchemaKind,
};
pub use visit::{
    BitField, ChildCall, DefinitionHeader, EnumSite, ExpressionReference, FieldOwner,
    GeometryAssignment, GeometryMember, GeometrySignature, PlacementProperty, RegisterLocation,
    RegisterNode, Renderer,
};

/// Built-in artifact renderers enabled by format-specific crate features.
#[cfg(any(feature = "export-c-header", feature = "export-veryl-layout"))]
pub mod backends;

/// One fully owned artifact emitted by an exporter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Application-defined logical output name with no filesystem meaning.
    pub logical_name: String,
    /// Application-defined media type describing `contents`.
    pub media_type: String,
    /// Fully owned output bytes.
    pub contents: Vec<u8>,
}

/// Receives artifacts from an exporter without imposing output-storage policy.
pub trait ArtifactSink {
    /// Error returned when this sink declines an artifact.
    type Error;

    /// Receives one owned artifact.
    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error>;
}

/// Drives every renderer through the same prepared definition traversal.
///
/// # Errors
/// Render failures emit no artifacts. A sink failure retains any artifacts it
/// previously accepted; storage rollback belongs to the sink.
pub fn generate<R: Renderer, S: ArtifactSink>(
    schema: &PreparedSchema,
    mut renderer: R,
    sink: &mut S,
) -> Result<(), ExportError<R::Error, S::Error>> {
    for definition in schema.definitions() {
        walk::definition(&mut renderer, definition)?;
    }
    for artifact in renderer.finish().map_err(ExportError::Render)? {
        sink.emit(artifact).map_err(ExportError::Sink)?;
    }
    Ok(())
}

/// Rendering or storage failure retaining the error owned by its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError<R, S> {
    /// Invalid expression scope encountered by the common walker.
    Layout(crate::graph::LayoutError),
    /// The renderer could not produce an artifact.
    Render(R),
    /// The sink rejected an artifact.
    Sink(S),
}

impl<R: fmt::Display, S: fmt::Display> fmt::Display for ExportError<R, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => error.fmt(f),
            Self::Render(error) => error.fmt(f),
            Self::Sink(error) => error.fmt(f),
        }
    }
}

impl<R: std::error::Error + 'static, S: std::error::Error + 'static> std::error::Error
    for ExportError<R, S>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Render(error) => Some(error),
            Self::Sink(error) => Some(error),
        }
    }
}

/// An infallible in-memory artifact collection.
#[derive(Debug, PartialEq, Eq)]
pub struct InMemoryArtifactSink {
    artifacts: Vec<Artifact>,
}

impl InMemoryArtifactSink {
    /// Creates an empty in-memory artifact collection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            artifacts: Vec::new(),
        }
    }

    /// Returns the collected artifacts in their emission order.
    #[must_use]
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Returns the collected artifacts, transferring their ownership to the caller.
    #[must_use]
    pub fn into_artifacts(self) -> Vec<Artifact> {
        self.artifacts
    }
}

impl Default for InMemoryArtifactSink {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactSink for InMemoryArtifactSink {
    type Error = Infallible;

    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error> {
        self.artifacts.push(artifact);
        Ok(())
    }
}
