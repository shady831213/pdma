use crate::SchemaCodecError;

/// Shared byte-layout contract implemented by generated records.
pub trait RecordLayout {
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
    /// Required byte alignment for this record.
    const ALIGNMENT: usize;
    /// Encoded byte extent of this record.
    const EXTENT: usize;
    /// All-zero owned value.
    const ZERO: Self::Value;

    /// Decodes one owned value from the start of `bytes`.
    fn decode(bytes: &[u8]) -> Result<Self::Value, SchemaCodecError>;
    /// Encodes one owned value at the start of `bytes`.
    fn encode(bytes: &mut [u8], value: &Self::Value) -> Result<(), SchemaCodecError>;
    /// Creates a checked immutable view.
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError>;
    /// Creates a checked mutable view.
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError>;
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
    /// Required byte alignment when this layout is placed in another map.
    const ALIGNMENT: usize;
    /// Encoded byte extent of this layout.
    const EXTENT: usize;
    /// All-zero owned value.
    const ZERO: Self::Value;
    /// Declared default value used when constructing this layout.
    const DEFAULT: Self::Value;

    /// Decodes one owned value from the start of `bytes`.
    fn decode(bytes: &[u8]) -> Result<Self::Value, SchemaCodecError>;
    /// Encodes one owned value at the start of `bytes`.
    fn encode(bytes: &mut [u8], value: &Self::Value) -> Result<(), SchemaCodecError>;
    /// Creates a checked immutable view.
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError>;
    /// Creates a checked mutable view.
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError>;
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
