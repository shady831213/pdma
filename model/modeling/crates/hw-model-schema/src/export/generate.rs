use super::{Context, Visit};
use super::{EntrySymbols, ExportError, PrepareEntries, value};
use crate::analysis::{AnalysisError, AnalyzedSchema, Unit};
use crate::traversal::NodeId;
use std::io::Write;
/// A source domain owns preflight and entry dispatch, independently of target syntax.
pub trait Domain<B: value::Backend>: super::context::sealed::Domain<B> {
    /// Dispatches an already bound visit without permitting raw-node rebinding.
    fn emit(
        visit: Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error>;
    /// Dispatches a defined value edge through its domain's existing reference callback.
    fn reference<'a>(
        reference: &'a crate::ast::Reference,
        ctx: B::Context<'a>,
        backend: &mut B,
        writer: &mut dyn Write,
    ) -> Result<(), B::Error>;
    /// Rejects an incompatible unit before target callbacks run.
    fn validate(unit: &Unit) -> Result<(), AnalysisError>;
    /// Binds selected entries to their completed analysis in dependency order.
    fn entries(schema: &AnalyzedSchema) -> impl Iterator<Item = Visit<B::Context<'_>>>;
    /// Invokes the entry callback with a framework-bound visit.
    fn entry(
        visit: Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error>;
}
/// One bound entry with target checks prepared before any output is written.
#[derive(Debug, Clone)]
pub struct PreparedEntry<C> {
    visit: Visit<C>,
}
impl<C: Context> PreparedEntry<C> {
    pub(super) const fn new(visit: Visit<C>) -> Self {
        Self { visit }
    }
    /// Source entry name for caller-owned routing, with no filename or MIME policy.
    pub fn name(&self) -> &str {
        self.visit.context().entry_name()
    }
    /// Stable identity of the bound semantic occurrence.
    #[must_use]
    pub fn id(&self) -> &NodeId {
        self.visit.context().id()
    }
    /// Emits this entry without flushing or publishing its destination.
    ///
    /// # Errors
    /// Returns the backend error; partial bytes remain in the writer on failure.
    pub fn emit<'a, B>(&self, backend: &mut B, writer: &mut dyn Write) -> Result<(), B::Error>
    where
        B: value::Backend<Context<'a> = C>,
    {
        B::Domain::entry(self.visit.clone(), backend, writer)
    }
}
/// Validates the entire domain and prepares every entry before writing any bytes.
///
/// # Errors
/// Returns incompatible-domain or backend target checks errors.
pub fn prepare<'a, B: value::Backend>(
    schema: &'a AnalyzedSchema,
    backend: &mut B,
) -> Result<Vec<PreparedEntry<B::Context<'a>>>, ExportError<B::Error>> {
    use crate::analysis::Pass;
    PrepareEntries.run((schema, backend, &mut EntrySymbols::new()))
}
/// Prepares explicitly selected bound visits, including entries of a mixed graph.
///
/// The visits already carry their domain and lexical binding. Their iteration
/// order is retained; no writer is opened and no bytes are produced here.
///
/// # Errors
/// Returns backend target checks errors before any prepared outputs are returned.
pub fn prepare_entries<'a, B: value::Backend>(
    backend: &mut B,
    entries: impl IntoIterator<Item = Visit<B::Context<'a>>>,
) -> Result<Vec<PreparedEntry<B::Context<'a>>>, ExportError<B::Error>> {
    EntrySymbols::new().prepare(backend, entries)
}
/// Prepares every entry, then concatenates their bytes into a caller-owned writer.
///
/// The caller owns buffering, flushing, staging and publication. Rendering errors
/// can leave partial bytes; target checks and domain errors occur before any writes.
///
/// # Errors
/// Returns domain-analysis or backend target checks/rendering errors.
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
