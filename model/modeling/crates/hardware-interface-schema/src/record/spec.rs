use crate::Documentation;

use super::{FieldSpec, RecordGraphError, RecordPlacement, RecordValidationError, ResolvedRecord};

/// Named, offset-free fixed-record definition.
#[derive(Debug, Clone, Copy)]
pub struct RecordSpec<I: 'static = u16> {
    documentation: Documentation,
    name: &'static str,
    size: usize,
    alignment: usize,
    fields: &'static [FieldSpec<I>],
    records: &'static [RecordPlacement<I>],
}

impl<I: Copy + Eq + core::fmt::Debug> RecordSpec<I> {
    /// Creates an unvalidated named record definition.
    pub const fn named(
        name: &'static str,
        size: usize,
        alignment: usize,
        fields: &'static [FieldSpec<I>],
        records: &'static [RecordPlacement<I>],
    ) -> Self {
        Self {
            name,
            documentation: Documentation::EMPTY,
            size,
            alignment,
            fields,
            records,
        }
    }

    /// Attaches documentation to this node.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns documentation with any referenced definition as fallback.
    pub const fn documentation(self) -> Documentation {
        self.documentation
    }

    /// Returns the definition name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the record width.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns the required alignment.
    pub const fn alignment(self) -> usize {
        self.alignment
    }

    /// Returns scalar placements in declaration storage order.
    pub const fn fields(self) -> &'static [FieldSpec<I>] {
        self.fields
    }

    /// Returns nested placements in declaration storage order.
    pub const fn records(self) -> &'static [RecordPlacement<I>] {
        self.records
    }

    /// Validates the local flat record using the historical six phases.
    pub fn validate(self) -> Result<(), RecordValidationError<I>> {
        super::validation::validate_flat(self)
    }

    /// Validates and deterministically resolves the complete record graph.
    pub fn resolve(self) -> Result<ResolvedRecord<I>, RecordGraphError<I>> {
        super::resolve::resolve(self)
    }
}

impl RecordSpec<u16> {
    /// Creates a legacy-compatible unnamed flat record definition.
    pub const fn new(size: usize, alignment: usize, fields: &'static [FieldSpec<u16>]) -> Self {
        Self::named("", size, alignment, fields, &[])
    }
}
