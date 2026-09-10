use crate::Documentation;

/// One named wire code in a reusable enum definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumVariantSpec {
    name: &'static str,
    value: u64,
    documentation: Documentation,
}

impl EnumVariantSpec {
    /// Creates a variant with its unsigned wire code.
    pub const fn new(name: &'static str, value: u64) -> Self {
        Self {
            name,
            value,
            documentation: Documentation::EMPTY,
        }
    }

    /// Attaches variant documentation.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns the declared variant name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the unsigned wire code.
    pub const fn value(self) -> u64 {
        self.value
    }

    /// Returns variant documentation.
    pub const fn documentation(self) -> Documentation {
        self.documentation
    }
}

/// Enum variants shared by scalar and bit-field definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumDefinition {
    variants: &'static [EnumVariantSpec],
}

impl EnumDefinition {
    /// Creates an enum definition in declaration order.
    pub const fn new(variants: &'static [EnumVariantSpec]) -> Self {
        Self { variants }
    }

    /// Returns named wire codes in declaration order.
    pub const fn variants(self) -> &'static [EnumVariantSpec] {
        self.variants
    }
}
