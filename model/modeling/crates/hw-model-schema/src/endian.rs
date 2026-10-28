/// Byte order used to encode a scalar value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}
