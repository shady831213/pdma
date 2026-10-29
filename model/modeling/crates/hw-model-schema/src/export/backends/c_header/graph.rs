use super::{CHeaderExportError, RecordCHeaderExporter, naming};
mod bits;
use crate::{
    RecordGraphError,
    export::{ArtifactSink, ExportError, Exporter, InMemoryArtifactSink},
    graph::{DefinitionBody, DefinitionId, SchemaGraph},
};
use std::collections::HashMap;

/// Failure rendering a definition graph; no unsupported definition is silently skipped.
#[derive(Debug)]
pub enum CHeaderGraphError {
    /// Formatting failed while building one artifact.
    Format(std::fmt::Error),
    /// Target-language symbol collision or fragment rendering error.
    Render(CHeaderExportError),
    /// Referenced record has invalid metadata.
    Layout {
        /// Canonical defining Rust path.
        path: &'static str,
        /// Original record validation error.
        source: RecordGraphError,
    },
    /// This record-graph backend requires register maps to be resolved and projected separately.
    RegisterDefinition(&'static str),
}
impl std::fmt::Display for CHeaderGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format(error) => error.fmt(f),
            Self::Render(error) => error.fmt(f),
            Self::Layout { path, source } => write!(f, "{path}: {source}"),
            Self::RegisterDefinition(path) => write!(
                f,
                "{path}: use RegisterMapCHeaderExporter on a resolved permission view"
            ),
        }
    }
}
impl std::error::Error for CHeaderGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Format(source) => Some(source),
            Self::Render(source) => Some(source),
            Self::Layout { source, .. } => Some(source),
            Self::RegisterDefinition(_) => None,
        }
    }
}

/// Renders every record/union in a graph using the existing C11 fragment format.
///
/// Default prefixes derive from declared names, not definition identity. Callers
/// can override names explicitly; collisions are diagnosed before any artifact
/// is emitted. Register maps use their separately resolved/projected views.
#[derive(Debug, Default)]
pub struct CHeaderExporter {
    names: HashMap<DefinitionId, String>,
}
impl CHeaderExporter {
    /// Creates a renderer with definition-derived names.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides a target symbol prefix without changing definition identity.
    pub fn set_name(&mut self, definition: DefinitionId, name: &str) {
        self.names.insert(definition, name.to_owned());
    }
}
impl Exporter<SchemaGraph> for CHeaderExporter {
    type Error = CHeaderGraphError;
    fn export<Sink: ArtifactSink>(
        &self,
        graph: &SchemaGraph,
        sink: &mut Sink,
    ) -> Result<(), ExportError<Self::Error, Sink::Error>> {
        let names: Vec<_> = graph
            .definitions()
            .map(|definition| {
                let name = self
                    .names
                    .get(&definition.id())
                    .cloned()
                    .unwrap_or_else(|| definition.name().to_ascii_lowercase());
                (
                    definition.id().path().to_owned(),
                    naming::c_identifier(&name),
                )
            })
            .collect();
        naming::reject_collisions(&names)
            .map_err(|error| ExportError::Render(CHeaderGraphError::Render(error)))?;
        let mut rendered = InMemoryArtifactSink::new();
        for definition in graph.definitions() {
            let name = self
                .names
                .get(&definition.id())
                .cloned()
                .unwrap_or_else(|| definition.name().to_ascii_lowercase());
            let spec = match definition.body() {
                DefinitionBody::Bits(layout) => {
                    let artifact = bits::render(&name, layout)
                        .map_err(|error| ExportError::Render(CHeaderGraphError::Format(error)))?;
                    match rendered.emit(artifact) {
                        Ok(()) => {}
                        Err(never) => match never {},
                    }
                    continue;
                }
                DefinitionBody::Record(spec) | DefinitionBody::Opaque(spec) => spec,
                DefinitionBody::Union { storage, .. } => storage,
                DefinitionBody::Register(_) | DefinitionBody::RegisterMap(_) => {
                    return Err(ExportError::Render(CHeaderGraphError::RegisterDefinition(
                        definition.id().path(),
                    )));
                }
            };
            let resolved = spec.resolve().map_err(|source| {
                ExportError::Render(CHeaderGraphError::Layout {
                    path: definition.id().path(),
                    source,
                })
            })?;
            let name = self
                .names
                .get(&definition.id())
                .cloned()
                .unwrap_or_else(|| definition.name().to_ascii_lowercase());
            match RecordCHeaderExporter::new(&name, &name).export(&resolved, &mut rendered) {
                Ok(()) => {}
                Err(ExportError::Render(error)) => {
                    return Err(ExportError::Render(CHeaderGraphError::Render(error)));
                }
                Err(ExportError::Sink(never)) => match never {},
            }
        }
        for artifact in rendered.into_artifacts() {
            sink.emit(artifact).map_err(ExportError::Sink)?;
        }
        Ok(())
    }
}
