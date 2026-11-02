use super::{ArtifactSink, ExportError, value};
use crate::analysis::{AnalysisError, AnalyzedSchema, Unit};
/// A source domain owns preflight and root dispatch, independently of target syntax.
pub trait Domain<B: value::Backend> {
    /// Dispatches an already bound visit without permitting raw-node rebinding.
    fn emit(visit: value::Visit<B::Context<'_>>, backend: &mut B) -> Result<(), B::Error>;
    /// Rejects an incompatible unit before target callbacks run.
    fn validate(unit: &Unit) -> Result<(), AnalysisError>;
    /// Binds selected roots to their completed analysis in dependency order.
    fn roots(schema: &AnalyzedSchema) -> impl Iterator<Item = value::Visit<B::Context<'_>>>;
    /// Invokes the root callback with a framework-bound visit.
    fn root(visit: value::Visit<B::Context<'_>>, backend: &mut B) -> Result<(), B::Error>;
}
/// Generates from a completed semantic result with one shared artifact lifecycle.
pub fn generate<B: value::Backend, S: ArtifactSink>(
    schema: &AnalyzedSchema,
    mut backend: B,
    sink: &mut S,
) -> Result<(), ExportError<B::Error, S::Error>> {
    for unit in schema.units() {
        B::Domain::validate(unit).map_err(ExportError::Analysis)?;
    }
    for visit in B::Domain::roots(schema) {
        B::Domain::root(visit, &mut backend).map_err(ExportError::Render)?;
    }
    for artifact in backend.finish().map_err(ExportError::Render)? {
        sink.emit(artifact).map_err(ExportError::Sink)?;
    }
    Ok(())
}
