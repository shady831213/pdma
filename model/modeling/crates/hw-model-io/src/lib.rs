#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// Byte-memory access interfaces, address vocabulary and supplied backing models.
pub mod memory;
/// Register access capabilities and ordered observation.
pub mod register;
