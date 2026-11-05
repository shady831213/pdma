use super::{ExportError, OutputMetadata, value};
use crate::analysis::{AnalysisError, AnalyzedSchema, Unit};
use crate::traversal::{Context, NodeId, Visit};
use std::io::Write;
/// A source domain owns preflight and root dispatch, independently of target syntax.
pub trait Domain<B: value::Backend> {
    /// Dispatches an already bound visit without permitting raw-node rebinding.
    fn emit(
        visit: Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error>;
    /// Rejects an incompatible unit before target callbacks run.
    fn validate(unit: &Unit) -> Result<(), AnalysisError>;
    /// Binds selected roots to their completed analysis in dependency order.
    fn roots(schema: &AnalyzedSchema) -> impl Iterator<Item = Visit<B::Context<'_>>>;
    /// Invokes the root callback with a framework-bound visit.
    fn root(
        visit: Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error>;
}
/// One bound root with metadata prepared before any output is written.
#[derive(Debug, Clone)]
pub struct PreparedOutput<C> {
    visit: Visit<C>,
    metadata: OutputMetadata,
}
impl<C: Context> PreparedOutput<C> {
    /// Identity used by callers to route this root to a destination.
    #[must_use]
    pub const fn metadata(&self) -> &OutputMetadata {
        &self.metadata
    }
    /// Stable identity of the bound semantic occurrence.
    #[must_use]
    pub fn id(&self) -> &NodeId {
        self.visit.context().id()
    }
    /// Emits this root without flushing or publishing its destination.
    ///
    /// # Errors
    /// Returns the backend error; partial bytes remain in the writer on failure.
    pub fn emit<'a, B>(&self, backend: &mut B, writer: &mut dyn Write) -> Result<(), B::Error>
    where
        B: value::Backend<Context<'a> = C>,
    {
        B::Domain::root(self.visit.clone(), backend, writer)
    }
}
/// Validates the entire domain and prepares every root before writing any bytes.
///
/// # Errors
/// Returns incompatible-domain or backend metadata errors.
pub fn prepare<'a, B: value::Backend>(
    schema: &'a AnalyzedSchema,
    backend: &mut B,
) -> Result<Vec<PreparedOutput<B::Context<'a>>>, ExportError<B::Error>> {
    for unit in schema.units() {
        B::Domain::validate(unit).map_err(ExportError::Analysis)?;
    }
    prepare_roots(backend, B::Domain::roots(schema))
}
/// Prepares explicitly selected bound visits, including roots of a mixed graph.
///
/// The visits already carry their domain and lexical binding. Their iteration
/// order is retained; no writer is opened and no bytes are produced here.
///
/// # Errors
/// Returns backend metadata errors before any prepared outputs are returned.
pub fn prepare_roots<'a, B: value::Backend>(
    backend: &mut B,
    roots: impl IntoIterator<Item = Visit<B::Context<'a>>>,
) -> Result<Vec<PreparedOutput<B::Context<'a>>>, ExportError<B::Error>> {
    roots
        .into_iter()
        .map(|visit| {
            let metadata = backend
                .metadata(visit.context())
                .map_err(ExportError::Render)?;
            Ok(PreparedOutput { visit, metadata })
        })
        .collect()
}
/// Prepares every root, then concatenates their bytes into a caller-owned writer.
///
/// The caller owns buffering, flushing, staging and publication. Rendering errors
/// can leave partial bytes; metadata and domain errors occur before any writes.
///
/// # Errors
/// Returns domain-analysis or backend metadata/rendering errors.
pub fn generate<B: value::Backend>(
    schema: &AnalyzedSchema,
    mut backend: B,
    writer: &mut dyn Write,
) -> Result<(), ExportError<B::Error>> {
    for output in prepare(schema, &mut backend)? {
        output
            .emit(&mut backend, writer)
            .map_err(ExportError::Render)?;
    }
    Ok(())
}

impl<D> crate::analysis::Pipeline<(&AnalyzedSchema, D)> {
    /// Constructs a backend borrowing prepared data, then runs ordinary generation.
    ///
    /// # Errors
    /// Returns the analysis or backend error from generation.
    pub fn generate<'b, B: value::Backend>(
        &'b self,
        backend: impl FnOnce(&'b D) -> B,
        writer: &mut dyn Write,
    ) -> Result<(), ExportError<B::Error>> {
        let (schema, data) = self.input();
        generate(schema, backend(data), writer)
    }
}
