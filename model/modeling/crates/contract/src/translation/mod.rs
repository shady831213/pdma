//! Explicit translations among semantic, exact ABI, and runtime address values.
//!
//! Translation functions own no bound device state or runtime effects.

/// Command translations that require runtime buffer-address context.
pub mod command;
/// Typed failures produced by cross-layer value translation.
pub mod error;
pub mod record;
mod status;

pub use command::{
    CopyBufferContext, CopyRuntimeAddresses, decode_copy_command, encode_copy_command,
};
pub use error::TranslationError;
pub use record::{decode_completion, decode_event, encode_completion, encode_event};

use crate::abi::{AbiDecodeError, AbiEncodeError, AbiId, DmaAddress};
use crate::semantic::{BufferId, FaultAddress};
use hardware_interface_runtime::MemoryAddress;

/// Converts an exact DMA address to semantic fault-address context.
pub const fn fault_address(address: DmaAddress) -> FaultAddress {
    FaultAddress::new(address.raw())
}

/// Converts semantic fault-address context to an exact DMA address.
pub const fn fault_dma_address(address: FaultAddress) -> DmaAddress {
    DmaAddress::new(address.raw())
}

/// Converts a runtime memory address to the fixed-width ABI address value.
pub const fn encode_memory_address(address: MemoryAddress) -> DmaAddress {
    DmaAddress::new(address.raw())
}

/// Converts a fixed-width ABI address to the runtime address value.
pub const fn decode_memory_address(address: DmaAddress) -> MemoryAddress {
    MemoryAddress::new(address.raw())
}

impl From<DmaAddress> for MemoryAddress {
    fn from(address: DmaAddress) -> Self {
        decode_memory_address(address)
    }
}

/// Checks an untrusted wide address before constructing a runtime address.
pub fn encode_memory_address_value(value: u128) -> Result<MemoryAddress, TranslationError> {
    DmaAddress::try_from(value)
        .map(decode_memory_address)
        .map_err(Into::into)
}

/// Checks an untrusted wide semantic identifier before constructing an ABI ID.
pub fn encode_id_value(value: u128) -> Result<AbiId, TranslationError> {
    AbiId::try_from(value).map_err(Into::into)
}

/// Converts one semantic buffer identity to its fixed-width ABI value.
pub fn encode_buffer_id(id: BufferId) -> Result<AbiId, AbiEncodeError> {
    AbiId::try_from(id.raw())
}

/// Converts one fixed-width ABI value to a semantic buffer identity.
pub fn decode_buffer_id(id: AbiId) -> Result<BufferId, AbiDecodeError> {
    id.try_into_model().map(BufferId::new)
}
