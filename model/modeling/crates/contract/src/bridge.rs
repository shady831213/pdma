//! Pure, explicit conversions between semantic and exact ABI values.
//!
//! Bridge functions perform typed value conversion only. Address lookup,
//! storage, synchronization, and runtime effects remain outside this crate.

mod command;
mod control;
mod error;
mod record;
mod ring;

pub use command::{
    CopyBufferContext, CopyRuntimeAddresses, decode_copy_command, encode_copy_command,
};
pub use control::{
    AbiDiscovery, AbiLimits, ControlRequest, DiscoveryRequest, QueueActivation, QueueControl,
    QueueCursorUpdate, QueueRingRegions,
};
pub use error::BridgeError;
pub use record::{
    AbiEvent, decode_completion, decode_event, encode_completion, encode_event,
    encode_semantic_event,
};
pub use ring::{RingCursors, RingLayout};

use crate::abi::{AbiAddressError, AbiDecodeError, AbiEncodeError, AbiId, DmaAddress};
use crate::semantic::{BufferId, FaultAddress};
use hardware_interface_runtime::MemoryAddress;

/// Converts an exact IOVA to semantic fault-address context.
pub const fn fault_address(address: DmaAddress) -> FaultAddress {
    FaultAddress::new(address.raw())
}

/// Converts semantic fault-address context to an exact IOVA.
pub const fn fault_iova(address: FaultAddress) -> DmaAddress {
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
pub fn encode_memory_address_value(value: u128) -> Result<MemoryAddress, BridgeError> {
    DmaAddress::try_from(value)
        .map(decode_memory_address)
        .map_err(Into::into)
}

/// Checks an untrusted wide semantic identifier before constructing an ABI ID.
pub fn encode_id_value(value: u128) -> Result<AbiId, BridgeError> {
    AbiId::try_from(value).map_err(Into::into)
}

/// Model correlation between a semantic buffer and its exact IOVA extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdmaMemoryRegion {
    object: BufferId,
    base: DmaAddress,
    length: u64,
    exclusive_end: DmaAddress,
}

impl PdmaMemoryRegion {
    /// Creates a region whose exclusive end is representable.
    pub const fn try_new(
        object: BufferId,
        base: DmaAddress,
        length: u64,
    ) -> Result<Self, AbiAddressError> {
        match base.checked_add(length) {
            Ok(exclusive_end) => Ok(Self {
                object,
                base,
                length,
                exclusive_end,
            }),
            Err(error) => Err(error),
        }
    }

    /// Returns the semantic DMA object.
    pub const fn object(self) -> BufferId {
        self.object
    }

    /// Returns the exact base IOVA.
    pub const fn base(self) -> DmaAddress {
        self.base
    }

    /// Returns the frozen ABI-v1 base address.
    pub const fn abi_base(self) -> DmaAddress {
        self.base
    }

    /// Returns the neutral runtime base address.
    pub const fn memory_base(self) -> MemoryAddress {
        decode_memory_address(self.base)
    }

    /// Returns the fixed-width byte length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Returns the checked exclusive end.
    pub const fn exclusive_end(self) -> DmaAddress {
        self.exclusive_end
    }

    /// Returns the frozen ABI-v1 exclusive end address.
    pub const fn abi_exclusive_end(self) -> DmaAddress {
        self.exclusive_end
    }

    /// Returns the neutral runtime exclusive end address.
    pub const fn memory_exclusive_end(self) -> MemoryAddress {
        decode_memory_address(self.exclusive_end)
    }

    /// Returns whether the region contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}

/// Converts one semantic buffer identity to its fixed-width ABI value.
pub fn encode_buffer_id(id: BufferId) -> Result<AbiId, AbiEncodeError> {
    AbiId::try_from(id.raw())
}

/// Converts one fixed-width ABI value to a semantic buffer identity.
pub fn decode_buffer_id(id: AbiId) -> Result<BufferId, AbiDecodeError> {
    id.try_into_model().map(BufferId::new)
}
