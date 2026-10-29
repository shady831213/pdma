use core::ops::Range;

use crate::{
    Documentation, Endian, RegisterValueError, SchemaCodecError, SchemaLocation,
    SchemaMemberIdentity,
};

/// Reusable bit-field shape without identity, name, or bit offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitsFieldDefinition {
    bit_width: usize,
    documentation: Documentation,
    enum_definition: Option<crate::EnumDefinition>,
}

impl BitsFieldDefinition {
    /// Creates a field definition with the declared number of bits.
    pub const fn new(bit_width: usize) -> Self {
        Self {
            bit_width,
            documentation: Documentation::EMPTY,
            enum_definition: None,
        }
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

    /// Returns the declared number of bits.
    pub const fn bit_width(self) -> usize {
        self.bit_width
    }

    /// Attaches the legal enum codes for this field.
    #[must_use]
    pub const fn with_enum(mut self, definition: crate::EnumDefinition) -> Self {
        self.enum_definition = Some(definition);
        self
    }

    /// Returns the optional enum definition.
    pub const fn enum_definition(self) -> Option<crate::EnumDefinition> {
        self.enum_definition
    }
}

/// One sibling-local placement in a reusable bits schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitsFieldSpec<I: 'static = u16> {
    documentation: Documentation,
    id: I,
    name: &'static str,
    lsb: usize,
    definition: BitsFieldDefinition,
    declaration_index: Option<usize>,
}

impl<I: Copy> BitsFieldSpec<I> {
    /// Places one reusable bit-field definition.
    pub const fn place(
        id: I,
        name: &'static str,
        lsb: usize,
        definition: BitsFieldDefinition,
    ) -> Self {
        Self {
            id,
            documentation: Documentation::EMPTY,
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

    /// Attaches placement documentation, overriding the referenced definition.
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
    pub const fn definition(self) -> BitsFieldDefinition {
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

    pub(crate) const fn declared_at(self, fallback: usize) -> usize {
        match self.declaration_index {
            Some(index) => index,
            None => fallback,
        }
    }
}

/// Overflow observed while encoding a typed bits value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitsValueError {
    identity: SchemaMemberIdentity,
    lsb: usize,
    bit_width: usize,
    value: u64,
    maximum: u64,
}

impl BitsValueError {
    /// Creates one bit-member overflow observation.
    pub const fn new(
        identity: SchemaMemberIdentity,
        lsb: usize,
        bit_width: usize,
        value: u64,
        maximum: u64,
    ) -> Self {
        Self {
            identity,
            lsb,
            bit_width,
            value,
            maximum,
        }
    }

    /// Returns the declared overflowing bit member.
    pub const fn identity(self) -> SchemaMemberIdentity {
        self.identity
    }

    /// Returns the member's least-significant local bit.
    pub const fn lsb(self) -> usize {
        self.lsb
    }

    /// Converts the observation for record encoding.
    pub const fn into_schema_error(self) -> SchemaCodecError {
        SchemaCodecError::NarrowingOverflow {
            location: self.identity.at_bit(self.lsb),
            observed: self.value,
            maximum: self.maximum,
        }
    }

    /// Converts the observation for register encoding.
    pub const fn into_register_error(self) -> RegisterValueError {
        RegisterValueError::FieldValueOverflow {
            location: self.identity.at_bit(self.lsb),
            bit_width: self.bit_width,
            value: self.value,
        }
    }
}

/// Shared contract implemented by markers generated with `bits_schema!`.
pub trait BitsLayout {
    /// Describes the shared bit definition independently of each embedding.
    fn schema_definition() -> crate::graph::Definition
    where
        Self: 'static,
    {
        crate::graph::Definition::bits::<Self>()
    }
    /// Owned typed value containing every declared bit field.
    type Value: Clone + Copy + Default + Eq + PartialEq;
    /// Unsigned scalar containing the complete encoded bit layout.
    type Storage: crate::Scalar;
    /// Stable field identity shared by every embedding.
    type FieldId: Copy + Eq + core::fmt::Debug + 'static;

    /// Stable schema name.
    const NAME: &'static str;
    /// Documentation of the reusable bits layout.
    const DOCUMENTATION: Documentation = Documentation::EMPTY;
    /// Highest exclusive declared bit endpoint.
    const BIT_WIDTH: usize;
    /// Smallest supported scalar byte extent containing every field.
    const EXTENT: usize;
    /// Mask containing every declared bit.
    const DECLARED_MASK: u64;
    /// Scalar metadata kind selected by the inferred extent.
    const SCALAR_KIND: crate::ScalarKind;
    /// Semantic default with zero integers and first-declared enum variants.
    const ZERO: Self::Value;
    /// Stable field metadata in declaration order.
    const FIELDS: &'static [BitsFieldSpec];

    /// Decodes declared fields from a register scalar while ignoring other bits.
    fn decode_register(raw: u64) -> Result<Self::Value, RegisterValueError>;

    /// Encodes declared fields into their register-local scalar positions.
    fn encode_register(value: &Self::Value) -> Result<u64, RegisterValueError>;

    /// Decodes one record field and rejects nonzero reserved bits.
    fn decode_record(
        bytes: &[u8],
        endian: Endian,
        location: SchemaLocation,
    ) -> Result<Self::Value, SchemaCodecError>;

    /// Encodes one record field using its containing record's byte order.
    fn encode_record(
        bytes: &mut [u8],
        endian: Endian,
        value: &Self::Value,
        location: SchemaLocation,
    ) -> Result<(), SchemaCodecError>;
}
