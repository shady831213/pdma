use crate::{Access, BitsFieldSpec, Documentation, Endian, RegisterDefinition, ResetKind};

/// Owned register metadata independent of application-specific field ID enums.
#[derive(Debug, Clone)]
pub struct RegisterInfo {
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
            })
            .collect();
        Self {
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
    /// Bit fields in declaration order, including enum semantics and descriptions.
    pub fn fields(&self) -> &[BitsFieldSpec<usize>] {
        &self.fields
    }
}
