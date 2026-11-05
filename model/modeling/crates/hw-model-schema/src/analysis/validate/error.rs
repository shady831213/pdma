/// Inconsistent source value metadata rejected before backend traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeError {
    /// The declared byte extent disagrees with its value type.
    Width {
        /// Declared byte extent.
        declared: usize,
        /// Required byte extent.
        expected: usize,
    },
    /// Scalar and embedding byte orders disagree.
    Endian,
    /// A record member refers to a non-value definition.
    NonValueReference,
    /// Invalid bit members, using the shared runtime validation contract.
    Bits(crate::RegisterDefinitionError<u16>),
    /// Cached bit extent or mask disagrees with the declared members.
    BitsMetadata,
    /// An enum field has no enumeration metadata.
    MissingEnumeration,
    /// An enumeration has no alternatives.
    EmptyEnumeration,
    /// Enumeration names or codes are repeated.
    DuplicateVariant,
    /// A constant or enumeration code exceeds its storage.
    ValueOverflow,
}
impl std::fmt::Display for ShapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Width { declared, expected } => {
                write!(f, "declared extent {declared}, expected {expected}")
            }
            Self::Bits(source) => source.fmt(f),
            Self::Endian => f.write_str("scalar and embedding byte orders differ"),
            Self::NonValueReference => f.write_str("record value refers to a non-value definition"),
            Self::BitsMetadata => f.write_str("bit extent or mask disagrees with members"),
            Self::MissingEnumeration => f.write_str("enum field lacks enumeration metadata"),
            Self::EmptyEnumeration => f.write_str("enumeration has no alternatives"),
            Self::DuplicateVariant => f.write_str("duplicate enumeration name or code"),
            Self::ValueOverflow => f.write_str("value exceeds its storage width"),
        }
    }
}
impl std::error::Error for ShapeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bits(source) => Some(source),
            Self::Width { .. }
            | Self::Endian
            | Self::NonValueReference
            | Self::BitsMetadata
            | Self::MissingEnumeration
            | Self::EmptyEnumeration
            | Self::DuplicateVariant
            | Self::ValueOverflow => None,
        }
    }
}
