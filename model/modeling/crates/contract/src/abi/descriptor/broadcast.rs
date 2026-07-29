use super::DescriptorValueError;
use crate::abi::{AbiId, DmaAddress, INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS};

/// Source and destination-list IOVAs for an indirect broadcast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastIovas {
    source: DmaAddress,
    destination_list: DmaAddress,
}

impl BroadcastIovas {
    /// Creates the absolute source and destination-list IOVAs.
    pub const fn new(source: DmaAddress, destination_list: DmaAddress) -> Self {
        Self {
            source,
            destination_list,
        }
    }
}

/// ABI-v1 indirect-broadcast command payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiBroadcastCopy {
    command_id: AbiId,
    iovas: BroadcastIovas,
    length: u32,
    destination_count: u32,
}

/// Fixed-width broadcast length and destination count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastShape {
    length: u32,
    destination_count: u32,
}

impl BroadcastShape {
    /// Creates broadcast scalar fields.
    pub const fn new(length: u32, destination_count: u32) -> Self {
        Self {
            length,
            destination_count,
        }
    }
}

impl AbiBroadcastCopy {
    /// Creates a broadcast command with a nonempty aligned destination list.
    ///
    /// # Errors
    /// Returns a typed error when the destination count is zero or the list is unaligned.
    pub const fn try_new(
        command_id: AbiId,
        iovas: BroadcastIovas,
        shape: BroadcastShape,
    ) -> Result<Self, DescriptorValueError> {
        if shape.destination_count == 0 {
            return Err(DescriptorValueError::InvalidIndirectCount {
                observed: shape.destination_count,
                maximum: MAX_INDIRECT_DESTINATIONS,
            });
        }
        if iovas.destination_list.raw() % INDIRECT_ENTRY_ALIGNMENT != 0 {
            return Err(DescriptorValueError::InvalidIndirectAlignment {
                address: iovas.destination_list.raw(),
                alignment: INDIRECT_ENTRY_ALIGNMENT,
            });
        }
        Ok(Self {
            command_id,
            iovas,
            length: shape.length,
            destination_count: shape.destination_count,
        })
    }

    /// Command identifier.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }

    /// Absolute source IOVA.
    pub const fn source(self) -> DmaAddress {
        self.iovas.source
    }

    /// Absolute destination-list IOVA.
    pub const fn destination_list(self) -> DmaAddress {
        self.iovas.destination_list
    }

    /// Bytes copied to every destination.
    pub const fn length(self) -> u32 {
        self.length
    }

    /// Number of destination-list entries.
    pub const fn destination_count(self) -> u32 {
        self.destination_count
    }

    /// Representable list extent in bytes.
    pub fn destination_list_extent(self) -> u64 {
        u64::from(self.destination_count) * INDIRECT_ENTRY_ALIGNMENT
    }
}

/// One ABI-v1 indirect-broadcast destination entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastDestinationEntry {
    address: DmaAddress,
}

impl BroadcastDestinationEntry {
    /// Creates an entry containing one absolute destination IOVA.
    pub const fn new(address: DmaAddress) -> Self {
        Self { address }
    }

    /// Absolute destination IOVA.
    pub const fn address(self) -> DmaAddress {
        self.address
    }
}
