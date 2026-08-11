use core::ops::Range;

/// Reusable bit-field shape without sibling identity, name, or bit offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterFieldDefinition {
    bit_width: usize,
}

impl RegisterFieldDefinition {
    /// Creates a field definition with the declared number of bits.
    pub const fn new(bit_width: usize) -> Self {
        Self { bit_width }
    }

    /// Returns the declared number of bits.
    pub const fn bit_width(self) -> usize {
        self.bit_width
    }
}

/// One sibling-local bit-field placement in a register definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterFieldSpec<I: 'static = u16> {
    id: I,
    name: &'static str,
    lsb: usize,
    definition: RegisterFieldDefinition,
    declaration_index: Option<usize>,
}

impl<I: Copy> RegisterFieldSpec<I> {
    /// Places one reusable bit-field definition.
    pub const fn place(
        id: I,
        name: &'static str,
        lsb: usize,
        definition: RegisterFieldDefinition,
    ) -> Self {
        Self {
            id,
            name,
            lsb,
            definition,
            declaration_index: None,
        }
    }

    /// Selects an explicit sibling declaration index.
    #[must_use]
    pub const fn with_declaration_index(mut self, index: usize) -> Self {
        self.declaration_index = Some(index);
        self
    }

    /// Returns the sibling identity.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the sibling name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the least-significant bit position.
    pub const fn lsb(self) -> usize {
        self.lsb
    }

    /// Returns the reusable field definition.
    pub const fn definition(self) -> RegisterFieldDefinition {
        self.definition
    }

    /// Returns the declared number of bits.
    pub const fn bit_width(self) -> usize {
        self.definition.bit_width()
    }

    /// Returns the checked local bit range.
    pub fn range(self) -> Option<Range<usize>> {
        self.lsb
            .checked_add(self.bit_width())
            .map(|end| self.lsb..end)
    }

    pub(super) const fn declared_at(self, fallback: usize) -> usize {
        match self.declaration_index {
            Some(index) => index,
            None => fallback,
        }
    }
}
