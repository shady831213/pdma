use crate::FieldSpec;

/// Physical field placement projected from a record's canonical storage.
#[derive(Debug, Clone, Copy)]
pub struct StorageField<'a>(pub(crate) &'a FieldSpec);
impl StorageField<'_> {
    /// Canonical physical field name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Byte offset relative to the containing record.
    pub const fn offset(self) -> usize {
        self.0.offset()
    }
    /// Encoded byte extent.
    pub const fn width(self) -> usize {
        self.0.width()
    }
    /// Placement documentation with definition fallback.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
}
