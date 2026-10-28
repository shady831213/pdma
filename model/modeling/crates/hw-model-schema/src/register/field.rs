/// Register-facing name for the shared bit-field shape.
pub type RegisterFieldDefinition = crate::BitsFieldDefinition;

/// Register-facing name for one shared bit-field placement.
pub type RegisterFieldSpec<I = u16> = crate::BitsFieldSpec<I>;
