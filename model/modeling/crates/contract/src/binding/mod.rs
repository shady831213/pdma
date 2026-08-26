//! Runtime-bound PDMA values that associate semantic identity with exact ABI state.

/// Runtime-bound control requests and queue activation values.
pub mod control;
/// Exact fault tuples associated with semantic command identity.
pub mod event;
/// Ring regions and cursors bound to semantic memory objects.
pub mod ring;

pub use control::{
    ControlRequest, QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions,
};
pub use event::AbiEvent;
pub use ring::{RingCursors, RingLayout};

use crate::abi::{AbiAddressError, DmaAddress};
use crate::semantic::BufferId;
use hardware_interface_runtime::MemoryAddress;

/// One checked association between a semantic object and its exact DMA extent.
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

    /// Returns the exact base DMA address.
    pub const fn base(self) -> DmaAddress {
        self.base
    }

    /// Returns the frozen ABI-v1 base address.
    pub const fn abi_base(self) -> DmaAddress {
        self.base
    }

    /// Returns the neutral runtime base address.
    pub const fn memory_base(self) -> MemoryAddress {
        MemoryAddress::new(self.base.raw())
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
        MemoryAddress::new(self.exclusive_end.raw())
    }

    /// Returns whether the region contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}
