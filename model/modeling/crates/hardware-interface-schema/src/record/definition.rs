use core::ops::Range;

use crate::{Endian, ScalarKind};

/// Reusable scalar shape without placement identity or offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDefinition {
    width: usize,
    scalar: ScalarKind,
    endian: Endian,
}

impl FieldDefinition {
    /// Creates a little-endian definition whose width matches `scalar`.
    pub const fn new(scalar: ScalarKind) -> Self {
        Self {
            width: scalar.width(),
            scalar,
            endian: Endian::Little,
        }
    }

    /// Selects the scalar byte order.
    #[must_use]
    pub const fn with_endian(mut self, endian: Endian) -> Self {
        self.endian = endian;
        self
    }

    /// Overrides the width for malformed-schema validation.
    #[must_use]
    pub const fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Returns the declared byte width.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Returns the scalar kind.
    pub const fn scalar(self) -> ScalarKind {
        self.scalar
    }

    /// Returns the scalar byte order.
    pub const fn endian(self) -> Endian {
        self.endian
    }
}

/// One sibling-local scalar placement in a record definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec<I: 'static = u16> {
    id: I,
    name: &'static str,
    offset: usize,
    definition: FieldDefinition,
    declaration_index: Option<usize>,
}

impl FieldSpec<u16> {
    /// Creates a legacy-compatible little-endian field placement.
    pub const fn new(id: u16, name: &'static str, offset: usize, scalar: ScalarKind) -> Self {
        Self::place(id, name, offset, FieldDefinition::new(scalar))
    }
}

impl<I: Copy> FieldSpec<I> {
    /// Places a reusable field definition.
    pub const fn place(
        id: I,
        name: &'static str,
        offset: usize,
        definition: FieldDefinition,
    ) -> Self {
        Self {
            id,
            name,
            offset,
            definition,
            declaration_index: None,
        }
    }

    /// Selects an explicit combined sibling declaration index.
    #[must_use]
    pub const fn with_declaration_index(mut self, index: usize) -> Self {
        self.declaration_index = Some(index);
        self
    }

    /// Replaces the generated sibling identifier during record composition.
    #[must_use]
    pub const fn with_id(mut self, id: I) -> Self {
        self.id = id;
        self
    }

    /// Replaces the sibling-local byte offset during record composition.
    #[must_use]
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Selects the field byte order.
    #[must_use]
    pub const fn with_endian(mut self, endian: Endian) -> Self {
        self.definition = self.definition.with_endian(endian);
        self
    }

    /// Overrides the width for malformed-schema validation.
    #[must_use]
    pub const fn with_width(mut self, width: usize) -> Self {
        self.definition = self.definition.with_width(width);
        self
    }

    /// Returns the stable sibling identifier.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the sibling name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the sibling-local byte offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the reusable scalar definition.
    pub const fn definition(self) -> FieldDefinition {
        self.definition
    }

    /// Returns the declared byte width.
    pub const fn width(self) -> usize {
        self.definition.width()
    }

    /// Returns the scalar kind.
    pub const fn scalar(self) -> ScalarKind {
        self.definition.scalar()
    }

    /// Returns the scalar byte order.
    pub const fn endian(self) -> Endian {
        self.definition.endian()
    }

    /// Returns the representable local byte range.
    pub fn range(self) -> Option<Range<usize>> {
        self.offset
            .checked_add(self.width())
            .map(|end| self.offset..end)
    }

    pub(super) const fn declared_at(self, fallback: usize) -> usize {
        match self.declaration_index {
            Some(index) => index,
            None => fallback,
        }
    }
}
