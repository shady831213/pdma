use crate::Endian;

use super::{
    Access, RegisterDefinitionError, RegisterFieldSpec, RegisterValueError, RegisterValueView,
    RegisterValueViewMut, ResetKind,
};

/// Named, offset-free whole-register definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterDefinition<I: 'static = u16> {
    name: &'static str,
    width: usize,
    alignment: usize,
    endian: Endian,
    access: Access,
    reset_kind: ResetKind,
    reset_value: u64,
    writable_mask: u64,
    fields: &'static [RegisterFieldSpec<I>],
}

impl<I: Copy> RegisterDefinition<I> {
    /// Creates a read-only zero-reset definition with no declared fields.
    pub const fn new(name: &'static str, width: usize, endian: Endian) -> Self {
        Self {
            name,
            width,
            alignment: width,
            endian,
            access: Access::ReadOnly,
            reset_kind: ResetKind::Zero,
            reset_value: 0,
            writable_mask: 0,
            fields: &[],
        }
    }

    /// Replaces the required byte alignment for placements of this definition.
    #[must_use]
    pub const fn with_alignment(mut self, alignment: usize) -> Self {
        self.alignment = alignment;
        self
    }

    /// Replaces the declared access permission.
    #[must_use]
    pub const fn with_access(mut self, access: Access) -> Self {
        self.access = access;
        self
    }

    /// Replaces the reset classification and value.
    #[must_use]
    pub const fn with_reset(mut self, kind: ResetKind, value: u64) -> Self {
        self.reset_kind = kind;
        self.reset_value = value;
        self
    }

    /// Replaces the writable-bit metadata.
    #[must_use]
    pub const fn with_writable_mask(mut self, mask: u64) -> Self {
        self.writable_mask = mask;
        self
    }

    /// Attaches sibling-local bit-field placements.
    #[must_use]
    pub const fn with_fields(mut self, fields: &'static [RegisterFieldSpec<I>]) -> Self {
        self.fields = fields;
        self
    }

    /// Returns the definition name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the byte width.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Returns the required byte alignment for placements of this definition.
    pub const fn alignment(self) -> usize {
        self.alignment
    }

    /// Returns the byte order.
    pub const fn endian(self) -> Endian {
        self.endian
    }

    /// Returns the access permission.
    pub const fn access(self) -> Access {
        self.access
    }

    /// Returns the reset classification.
    pub const fn reset_kind(self) -> ResetKind {
        self.reset_kind
    }

    /// Returns the reset value.
    pub const fn reset_value(self) -> u64 {
        self.reset_value
    }

    /// Returns the writable mask.
    pub const fn writable_mask(self) -> u64 {
        self.writable_mask
    }

    /// Returns bit-field placements in storage order.
    pub const fn fields(self) -> &'static [RegisterFieldSpec<I>] {
        self.fields
    }

    /// Creates a checked immutable whole-register value view.
    pub fn view(self, bytes: &[u8]) -> Result<RegisterValueView<'_, I>, RegisterValueError> {
        RegisterValueView::new(self, bytes)
    }

    /// Creates a checked mutable whole-register value view.
    pub fn view_mut(
        self,
        bytes: &mut [u8],
    ) -> Result<RegisterValueViewMut<'_, I>, RegisterValueError> {
        RegisterValueViewMut::new(self, bytes)
    }
}

impl<I: Copy + Eq + core::fmt::Debug> RegisterDefinition<I> {
    /// Validates the definition in the documented total order.
    pub fn validate(self) -> Result<(), RegisterDefinitionError<I>> {
        super::validation::validate_definition(self, self.name, self.name, 0)
    }

    pub(crate) fn validate_at(
        self,
        root: &'static str,
        path: &str,
        declaration_index: usize,
    ) -> Result<(), RegisterDefinitionError<I>> {
        super::validation::validate_definition(self, root, path, declaration_index)
    }
}
