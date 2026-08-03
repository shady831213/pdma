/// Encodes one 32-bit scalar in MMIO little-endian order.
pub const fn encode_u32(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

/// Encodes one 64-bit scalar in MMIO little-endian order.
pub const fn encode_u64(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

/// Decodes exactly four MMIO little-endian bytes.
pub const fn decode_u32(bytes: [u8; 4]) -> u32 {
    u32::from_le_bytes(bytes)
}

/// Decodes exactly eight MMIO little-endian bytes.
pub const fn decode_u64(bytes: [u8; 8]) -> u64 {
    u64::from_le_bytes(bytes)
}
