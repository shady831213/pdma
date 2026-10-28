use crate::{
    Documentation, RegisterCollection, RegisterDefinition, SchemaCodecError, SchemaCoordinate,
};
mod bank;
pub use bank::{
    RegisterBank, RegisterBankError, RegisterShape, StaticRegisterLayout, checked_bank_offset,
};

/// Byte layout, metadata and checked views independent of codec policy.
///
/// Generated records and untagged unions implement this trait. Use it when only
/// geometry or views are needed; require [`SchemaCodec`] when an owned value must
/// be selected and decoded. An untagged union has a layout even when the application
/// has not supplied a rule for choosing its active variant.
pub trait SchemaLayout {
    /// Owned decoded value for this layout.
    type Value: Clone + Copy + Default + Eq + PartialEq;
    /// Checked immutable view for this layout.
    type View<'a>
    where
        Self: 'a;
    /// Checked mutable view for this layout.
    type ViewMut<'a>
    where
        Self: 'a;

    /// Stable layout name.
    const NAME: &'static str;
    /// Documentation of the reusable layout.
    const DOCUMENTATION: Documentation = Documentation::EMPTY;
    /// Required byte alignment for this layout.
    const ALIGNMENT: usize;
    /// Encoded byte extent of this layout.
    const EXTENT: usize;
    /// Optional structural metadata; opaque layouts need not describe their internals.
    const SPEC: crate::RecordSpec =
        crate::RecordSpec::named(Self::NAME, Self::EXTENT, Self::ALIGNMENT, &[], &[])
            .with_documentation(Self::DOCUMENTATION);
    /// All-zero owned value.
    const ZERO: Self::Value;

    /// Checks direct padding when selected as a complete untagged union case.
    /// Ordinary record decoding does not invoke this hook. Opaque codecs own
    /// their validation in `decode`; metadata must never infer their padding.
    fn validate_padding(_bytes: &[u8]) -> Result<(), SchemaCodecError> {
        Ok(())
    }
    /// Creates a checked immutable view.
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError>;
    /// Creates a checked mutable view.
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError>;
}

/// Composable byte codec implemented by generated records and application types.
///
/// The layout already supplies the value type, geometry and views. Generated
/// records implement both traits. For an untagged union, implement only `decode`
/// and `encode` here using its generated variant helpers and the application's
/// selection rule. Nested codecs then compose through the same trait; schema does
/// not infer protocol tags from arbitrary bytes.
///
/// A custom Rust type may also participate by implementing [`SchemaLayout`] first.
/// Errors should retain [`SchemaCodecError`]'s location information so enclosing
/// codecs can report failures relative to the composed layout.
pub trait SchemaCodec: SchemaLayout {
    /// Decodes one owned value from the start of `bytes`.
    ///
    /// # Errors
    /// Returns [`SchemaCodecError`] when bytes are insufficient or rejected by the
    /// codec's scalar, variant-selection or value-validation rules.
    fn decode(bytes: &[u8]) -> Result<Self::Value, SchemaCodecError>;
    /// Encodes one owned value at the start of `bytes`.
    ///
    /// # Errors
    /// Returns [`SchemaCodecError`] for an insufficient destination or a value
    /// rejected by the codec. A custom implementation defines its failure effects;
    /// callers must not infer transactional writes from this trait alone.
    fn encode(bytes: &mut [u8], value: &Self::Value) -> Result<(), SchemaCodecError>;
}

/// Shared byte-layout contract implemented by generated registers and register maps.
pub trait RegisterLayout {
    /// Owned decoded value for this layout.
    type Value: Clone + Copy + Default + Eq + PartialEq;
    /// Checked immutable view for this layout.
    type View<'a>
    where
        Self: 'a;
    /// Checked mutable view for this layout.
    type ViewMut<'a>
    where
        Self: 'a;

    /// Stable layout name.
    const NAME: &'static str;
    /// Documentation of the reusable register or map layout.
    const DOCUMENTATION: Documentation = Documentation::EMPTY;
    /// Required byte alignment when this layout is placed in another map.
    const ALIGNMENT: usize;
    /// Encoded byte extent of this layout.
    const EXTENT: usize;
    /// All-zero owned value.
    const ZERO: Self::Value;
    /// Declared default value used when constructing this layout.
    const DEFAULT: Self::Value;

    /// Reads the full scalar only at an exact leaf-register start.
    /// This owned-value operation does not apply bus access permissions.
    fn read_register(value: &Self::Value, offset: usize) -> Option<u64>;
    /// Replaces one leaf scalar, returning `None` without mutation for an unknown
    /// start or a value exceeding its width. Bus permissions and masks are not applied.
    fn write_register(value: &mut Self::Value, offset: usize, raw: u64) -> Option<()>;

    /// Decodes one owned value from the start of `bytes`.
    fn decode(bytes: &[u8]) -> Result<Self::Value, SchemaCodecError>;
    /// Encodes one owned value at the start of `bytes`.
    fn encode(bytes: &mut [u8], value: &Self::Value) -> Result<(), SchemaCodecError>;
    /// Creates a checked immutable view.
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError>;
    /// Creates a checked mutable view.
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError>;

    /// Creates an immutable view whose member errors are relative to `base`.
    #[doc(hidden)]
    fn view_at(bytes: &[u8], base: SchemaCoordinate) -> Result<Self::View<'_>, SchemaCodecError> {
        Self::view(bytes).map_err(|error| error.rebased(base))
    }

    /// Creates a mutable view whose member errors are relative to `base`.
    #[doc(hidden)]
    fn view_mut_at(
        bytes: &mut [u8],
        base: SchemaCoordinate,
    ) -> Result<Self::ViewMut<'_>, SchemaCodecError> {
        Self::view_mut(bytes).map_err(|error| error.rebased(base))
    }

    /// Appends every leaf register below one placement to a resolved map.
    #[doc(hidden)]
    fn append_registers<I: Copy + 'static>(_collection: RegisterCollection<'_, I>) {}
}

/// Runtime-neutral exact-start metadata for a generated register map.
pub trait RegisterMapLayout: RegisterLayout {
    /// Returns the leaf definition only when `offset` is its exact start.
    fn register_definition(offset: usize) -> Option<RegisterDefinition<u16>>;
}

/// Computes the next aligned offset without wrapping.
pub const fn checked_align_up(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return None;
    }
    let mask = alignment - 1;
    match value.checked_add(mask) {
        Some(sum) => Some(sum & !mask),
        None => None,
    }
}

/// Computes an inferred aligned offset, returning `usize::MAX` on invalid geometry.
pub const fn inferred_align_up(value: usize, alignment: usize) -> usize {
    match checked_align_up(value, alignment) {
        Some(aligned) => aligned,
        None => usize::MAX,
    }
}

/// Converts a zero-based declaration index into its one-based generated field ID.
pub const fn inferred_field_id(index: usize) -> Option<u16> {
    let Some(value) = index.checked_add(1) else {
        return None;
    };
    if value > 65_535 {
        return None;
    }
    let bytes = value.to_le_bytes();
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Computes the last byte after a repeated placement without wrapping.
pub const fn checked_placement_end(
    offset: usize,
    count: usize,
    stride: usize,
    extent: usize,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match (count - 1).checked_mul(stride) {
        Some(step) => match offset.checked_add(step) {
            Some(last) => last.checked_add(extent),
            None => None,
        },
        None => None,
    }
}
