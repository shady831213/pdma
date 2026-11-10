use super::{ExportError, PreparedEntry, SymbolCollision, SymbolScope, Visit, value};

/// One target namespace shared by all entry renderers contributing to an output.
/// This stores language symbols and diagnostic provenance, not files or media types.
#[derive(Debug, Default)]
pub struct EntrySymbols {
    declarations: SymbolScope<(usize, String)>,
    next: usize,
}
impl EntrySymbols {
    /// Starts an empty target namespace.
    pub const fn new() -> Self {
        Self {
            declarations: SymbolScope::new(),
            next: 0,
        }
    }
    /// Claims a target comparison key using its source diagnostic path.
    pub fn declare(
        &mut self,
        key: &str,
        source: &str,
    ) -> Result<(), SymbolCollision<(usize, String)>> {
        self.declarations.insert(key, (self.next, source.into()))?;
        self.next += 1;
        Ok(())
    }
    /// Runs target entry checks before any rendering. A generator can share this
    /// namespace between record and register backends without rebuilding names.
    pub fn prepare<'a, B: value::Backend>(
        &mut self,
        backend: &mut B,
        entries: impl IntoIterator<Item = Visit<B::Context<'a>>>,
    ) -> Result<Vec<PreparedEntry<B::Context<'a>>>, ExportError<B::Error>> {
        entries
            .into_iter()
            .map(|visit| {
                backend
                    .check_entry(visit.context(), self)
                    .map_err(ExportError::Render)?;
                Ok(PreparedEntry::new(visit))
            })
            .collect()
    }
}

/// Naming/preflight stage for ordinary typed analysis pipelines.
#[derive(Debug, Clone, Copy)]
pub struct PrepareEntries;
impl<'a, B: value::Backend>
    crate::analysis::Pass<(
        &'a crate::analysis::AnalyzedSchema,
        &mut B,
        &mut EntrySymbols,
    )> for PrepareEntries
{
    type Output = Vec<PreparedEntry<B::Context<'a>>>;
    type Error = ExportError<B::Error>;
    fn run(
        self,
        (schema, backend, symbols): (
            &'a crate::analysis::AnalyzedSchema,
            &mut B,
            &mut EntrySymbols,
        ),
    ) -> Result<Self::Output, Self::Error> {
        use super::Domain;
        for unit in schema.units() {
            B::Domain::validate(unit).map_err(ExportError::Analysis)?;
        }
        symbols.prepare(backend, B::Domain::entries(schema))
    }
}
