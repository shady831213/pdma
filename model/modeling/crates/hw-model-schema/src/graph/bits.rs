use crate::{BitsFieldSpec, BitsLayout, ScalarKind};

/// A reusable scalar bit layout has no intrinsic byte order; its embedding owns that choice.
#[derive(Debug, Clone)]
pub struct BitsDefinition {
    scalar: ScalarKind,
    bit_width: usize,
    declared_mask: u64,
    fields: &'static [BitsFieldSpec],
}
impl BitsDefinition {
    /// Captures the metadata already supplied by a typed bits layout.
    pub const fn of<T: BitsLayout + ?Sized>() -> Self {
        Self {
            scalar: T::SCALAR_KIND,
            bit_width: T::BIT_WIDTH,
            declared_mask: T::DECLARED_MASK,
            fields: T::FIELDS,
        }
    }
    /// Scalar storage kind, independent of memory byte order.
    pub const fn scalar(&self) -> ScalarKind {
        self.scalar
    }
    /// Highest exclusive declared bit endpoint.
    pub const fn bit_width(&self) -> usize {
        self.bit_width
    }
    /// Union of declared field masks.
    pub const fn declared_mask(&self) -> u64 {
        self.declared_mask
    }
    /// Ordered fields, including enum definitions and descriptions.
    pub const fn fields(&self) -> &'static [BitsFieldSpec] {
        self.fields
    }
}
