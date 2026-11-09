mod node;
mod semantic;
use crate::{Access, BitsFieldSpec, Documentation, Endian, RegisterDefinition, ResetKind};
pub use node::Node;
pub use semantic::{Argument, Attribute, LayoutProperty, LayoutPropertyKind, Parameter};

/// Owned register metadata independent of application-specific field ID enums.
#[derive(Debug, Clone)]
pub struct RegisterInfo {
    source: Option<super::Reference>,
    inline_bits: Option<super::BitsDefinition>,
    name: &'static str,
    documentation: Documentation,
    width: usize,
    alignment: usize,
    endian: Endian,
    access: Access,
    reset_kind: ResetKind,
    reset_value: u64,
    writable_mask: u64,
    fields: Vec<BitsFieldSpec<usize>>,
}
impl RegisterInfo {
    /// Preserves declaration order, semantic bit fields and all access metadata.
    pub fn from_spec<I: Copy>(spec: RegisterDefinition<I>) -> Self {
        let fields = spec
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                BitsFieldSpec::place(index, field.name(), field.lsb(), field.definition())
                    .with_documentation(field.documentation())
                    .with_declaration_index(field.declared_at(index))
            })
            .collect();
        Self {
            source: None,
            inline_bits: None,
            name: spec.name(),
            documentation: spec.documentation(),
            width: spec.width(),
            alignment: spec.alignment(),
            endian: spec.endian(),
            access: spec.access(),
            reset_kind: spec.reset_kind(),
            reset_value: spec.reset_value(),
            writable_mask: spec.writable_mask(),
            fields,
        }
    }
    /// Attaches a reused bits definition instead of inline source fields.
    #[must_use]
    pub fn with_source(mut self, source: super::Reference) -> Self {
        self.source = Some(source);
        self.inline_bits = None;
        self
    }
    pub(crate) fn rewritten_source(&self, source: Option<super::Reference>) -> Self {
        Self {
            source,
            inline_bits: None,
            ..self.clone()
        }
    }
    /// Attaches an owned bits declaration without changing register access or encoding.
    #[must_use]
    pub fn with_inline_bits(&self, bits: super::BitsDefinition) -> Self {
        Self {
            source: None,
            inline_bits: Some(bits),
            ..self.clone()
        }
    }
    /// Explicit owned bits source, when a reusable source has been expanded.
    pub const fn inline_bits(&self) -> Option<&super::BitsDefinition> {
        self.inline_bits.as_ref()
    }
    /// Source bits reference, absent when fields were declared inline.
    pub const fn source(&self) -> Option<&super::Reference> {
        self.source.as_ref()
    }
    /// Declared name.
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Description of the register.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }
    /// Encoded width in bytes.
    pub const fn width(&self) -> usize {
        self.width
    }
    /// Required placement alignment.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }
    /// Byte order.
    pub const fn endian(&self) -> Endian {
        self.endian
    }
    /// Bus access permissions.
    pub const fn access(&self) -> Access {
        self.access
    }
    /// Reset ownership/classification.
    pub const fn reset_kind(&self) -> ResetKind {
        self.reset_kind
    }
    /// Declared reset value.
    pub const fn reset_value(&self) -> u64 {
        self.reset_value
    }
    /// Writable bit mask.
    pub const fn writable_mask(&self) -> u64 {
        self.writable_mask
    }
    /// Validates target-independent register shape through the runtime definition validator.
    ///
    /// # Errors
    /// Rejects invalid register geometry, field declarations, writable masks or reset values.
    pub fn validate_shape(&self) -> Result<(), crate::RegisterDefinitionError<usize>> {
        RegisterDefinition::<usize>::new(self.name, self.width, self.endian)
            .with_alignment(self.alignment)
            .with_writable_mask(self.writable_mask)
            .with_reset(self.reset_kind, self.reset_value)
            .validate_fields(&self.fields)
    }
    /// Bit fields in declaration order, including enum semantics and descriptions.
    pub fn fields(&self) -> &[BitsFieldSpec<usize>] {
        &self.fields
    }
}
