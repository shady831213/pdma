/// Optional text attached to a schema definition or placement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Documentation {
    /// Concise summary of the node.
    pub short_description: Option<&'static str>,
    /// Detailed explanation of the node.
    pub description: Option<&'static str>,
}

impl Documentation {
    /// Metadata with neither text value specified.
    pub const EMPTY: Self = Self {
        short_description: None,
        description: None,
    };

    /// Inherits missing values while preserving explicit empty strings.
    #[must_use]
    pub const fn with_fallback(self, fallback: Self) -> Self {
        Self {
            short_description: match self.short_description {
                Some(value) => Some(value),
                None => fallback.short_description,
            },
            description: match self.description {
                Some(value) => Some(value),
                None => fallback.description,
            },
        }
    }
}
