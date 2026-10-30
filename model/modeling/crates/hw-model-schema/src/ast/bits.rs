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
    /// Constructs an inline source bits declaration from its original members.
    pub fn new(scalar: ScalarKind, fields: &'static [BitsFieldSpec]) -> Self {
        let mut bit_width = 0;
        let mut declared_mask = 0_u64;
        for field in fields {
            bit_width = bit_width.max(field.lsb().saturating_add(field.bit_width()));
            let mask = if field.bit_width() >= 64 {
                u64::MAX
            } else {
                (1_u64 << field.bit_width()) - 1
            };
            declared_mask |= mask
                .checked_shl(u32::try_from(field.lsb()).map_or(u32::MAX, |value| value))
                .map_or(0, |value| value);
        }
        Self {
            scalar,
            bit_width,
            declared_mask,
            fields,
        }
    }

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
