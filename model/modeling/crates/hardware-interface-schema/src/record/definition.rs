use core::ops::Range;

use crate::{BitsFieldSpec, Documentation, Endian, EnumDefinition, ScalarKind};

/// Reusable scalar shape without placement identity or offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDefinition {
    enum_definition: Option<EnumDefinition>,
    bits: &'static [BitsFieldSpec],
    documentation: Documentation,
    width: usize,
    scalar: ScalarKind,
    endian: Endian,
}

impl FieldDefinition {
    /// Creates a little-endian definition whose width matches `scalar`.
    pub const fn new(scalar: ScalarKind) -> Self {
        Self {
            enum_definition: None,
            width: scalar.width(),
            bits: &[],
            documentation: Documentation::EMPTY,
            scalar,
            endian: Endian::Little,
        }
    }

    /// Attaches the enum's named wire codes.
    #[must_use]
    pub const fn with_enum(mut self, definition: EnumDefinition) -> Self {
        self.enum_definition = Some(definition);
        self
    }

    /// Returns the scalar's enum metadata, when present.
    pub const fn enum_definition(self) -> Option<EnumDefinition> {
        self.enum_definition
    }

    /// Attaches definition documentation.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns definition documentation.
    pub const fn documentation(self) -> Documentation {
        self.documentation
    }

    /// Attaches the declared bit members embedded in this scalar.
    #[must_use]
    pub const fn with_bits(mut self, bits: &'static [BitsFieldSpec]) -> Self {
        self.bits = bits;
        self
    }

    /// Returns embedded bit members in declaration order.
    pub const fn bits(self) -> &'static [BitsFieldSpec] {
        self.bits
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
    documentation: Documentation,
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
    /// Attaches the enum's named wire codes.
    #[must_use]
    pub const fn with_enum(mut self, definition: EnumDefinition) -> Self {
        self.definition = self.definition.with_enum(definition);
        self
    }

    /// Returns the scalar's enum metadata, when present.
    pub const fn enum_definition(self) -> Option<EnumDefinition> {
        self.definition.enum_definition()
    }

    /// Places a reusable field definition.
    pub const fn place(
        id: I,
        name: &'static str,
        offset: usize,
        definition: FieldDefinition,
    ) -> Self {
        Self {
            id,
            documentation: Documentation::EMPTY,
            name,
            offset,
            definition,
            declaration_index: None,
        }
    }

    /// Attaches placement documentation.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns placement documentation with definition fallback.
    pub const fn documentation(self) -> Documentation {
        self.documentation
            .with_fallback(self.definition.documentation())
    }

    /// Attaches the declared bit members embedded in this scalar.
    #[must_use]
    pub const fn with_bits(mut self, bits: &'static [BitsFieldSpec]) -> Self {
        self.definition = self.definition.with_bits(bits);
        self
    }

    /// Returns embedded bit members in declaration order.
    pub const fn bits(self) -> &'static [BitsFieldSpec] {
        self.definition.bits()
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
