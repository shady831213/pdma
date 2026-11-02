use crate::Endian;
/// Source record/field geometry and byte-order attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    /// Occupied byte size.
    Size(usize),
    /// Source alignment when explicitly declared.
    Alignment(usize),
    /// Byte offset within the enclosing record.
    Offset(usize),
    /// Declared byte order.
    Endian(Endian),
}
