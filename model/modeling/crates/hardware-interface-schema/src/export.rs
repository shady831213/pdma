//! Format-neutral, owned artifact export vocabulary.

use core::convert::Infallible;

/// Built-in artifact renderers enabled by format-specific crate features.
#[cfg(feature = "export-c-header")]
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

/// Renders one schema value into artifacts through an application-provided sink.
pub trait Exporter<S> {
    /// Error returned by this exporter or its compatible sink.
    type Error;

    /// Emits this exporter's artifacts for `schema` into `sink`.
    fn export<Sink>(&self, schema: &S, sink: &mut Sink) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>;
}

/// An in-memory sink parameterized by an exporter's error type.
#[derive(Debug, PartialEq, Eq)]
pub struct InMemoryArtifactSink<E = Infallible> {
    artifacts: Vec<Artifact>,
    error: core::marker::PhantomData<fn() -> E>,
}

impl<E> InMemoryArtifactSink<E> {
    /// Creates an empty in-memory artifact collection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            artifacts: Vec::new(),
            error: core::marker::PhantomData,
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

impl<E> Default for InMemoryArtifactSink<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> ArtifactSink for InMemoryArtifactSink<E> {
    type Error = E;

    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error> {
        self.artifacts.push(artifact);
        Ok(())
    }
}
