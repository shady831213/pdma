use crate::Endian;

use super::{RegisterDefinition, RegisterValidationError};

/// Declared software access permission for a register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Reads are permitted and writes are not.
    ReadOnly,
    /// Reads and writes are permitted.
    ReadWrite,
    /// Writes are permitted and reads are not.
    WriteOnly,
}

/// Application-defined reset classification attached to a register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetKind {
    /// The declared reset value is constant.
    Constant,
    /// The application classifies the register as zero-reset.
    Zero,
    /// The application populates the register after reset.
    PostReset,
}

/// One map placement carrying identity, name, absolute or bank-relative offset, and definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterSpec<I: 'static, F: 'static = u16> {
    id: I,
    numeric_id: Option<u16>,
    name: &'static str,
    offset: usize,
    definition: RegisterDefinition<F>,
    declaration_index: Option<usize>,
}

impl<I: Copy> RegisterSpec<I, u16> {
    /// Creates a legacy-compatible placement with an inline little-endian definition.
    pub const fn new(id: I, name: &'static str, offset: usize, width: usize) -> Self {
        Self::place(
            id,
            name,
            offset,
            RegisterDefinition::new(name, width, Endian::Little),
        )
    }
}

impl<I: Copy, F: Copy> RegisterSpec<I, F> {
    /// Places an independently reusable register definition.
    pub const fn place(
        id: I,
        name: &'static str,
        offset: usize,
        definition: RegisterDefinition<F>,
    ) -> Self {
        Self {
            id,
            numeric_id: None,
            name,
            offset,
            definition,
            declaration_index: None,
        }
    }

    /// Attaches the explicit numeric identity from a schema declaration.
    #[must_use]
    pub const fn with_numeric_id(mut self, id: u16) -> Self {
        self.numeric_id = Some(id);
        self
    }

    /// Selects an explicit map declaration index.
    #[must_use]
    pub const fn with_declaration_index(mut self, index: usize) -> Self {
        self.declaration_index = Some(index);
        self
    }

    /// Replaces the declared access permission on the inline definition.
    #[must_use]
    pub const fn with_access(mut self, access: Access) -> Self {
        self.definition = self.definition.with_access(access);
        self
    }

    /// Replaces reset metadata on the inline definition.
    #[must_use]
    pub const fn with_reset(mut self, kind: ResetKind, value: u64) -> Self {
        self.definition = self.definition.with_reset(kind, value);
        self
    }

    /// Replaces writable-bit metadata on the inline definition.
    #[must_use]
    pub const fn with_writable_mask(mut self, mask: u64) -> Self {
        self.definition = self.definition.with_writable_mask(mask);
        self
    }

    /// Returns the caller-defined map identity.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the explicit numeric schema identity, when declared.
    pub const fn numeric_id(self) -> Option<u16> {
        self.numeric_id
    }

    /// Returns the placement name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the absolute or bank-relative byte offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the referenced offset-free definition.
    pub const fn definition(self) -> RegisterDefinition<F> {
        self.definition
    }

    /// Returns the definition byte width.
    pub const fn width(self) -> usize {
        self.definition.width()
    }

    /// Returns the definition access permission.
    pub const fn access(self) -> Access {
        self.definition.access()
    }

    /// Returns the definition reset classification.
    pub const fn reset_kind(self) -> ResetKind {
        self.definition.reset_kind()
    }

    /// Returns the definition reset value.
    pub const fn reset_value(self) -> u64 {
        self.definition.reset_value()
    }

    /// Returns the definition writable mask.
    pub const fn writable_mask(self) -> u64 {
        self.definition.writable_mask()
    }

    /// Validates legacy whole-register metadata.
    pub const fn validate(self) -> Result<(), RegisterValidationError> {
        super::validation::validate_legacy(
            self.name,
            self.width(),
            self.writable_mask(),
            self.reset_value(),
        )
    }

    pub(crate) const fn declared_at(self, fallback: usize) -> usize {
        match self.declaration_index {
            Some(index) => index,
            None => fallback,
        }
    }
}
