use crate::abi::{AbiId, DmaAddress};

/// Source and destination DMA addresses for vector addition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorDmaAddresses {
    source_a: DmaAddress,
    source_b: DmaAddress,
    destination: DmaAddress,
}

impl VectorDmaAddresses {
    /// Creates three absolute vector DMA addresses.
    pub const fn new(source_a: DmaAddress, source_b: DmaAddress, destination: DmaAddress) -> Self {
        Self {
            source_a,
            source_b,
            destination,
        }
    }
}

/// ABI-v1 byte-wise vector-add command payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiVectorAdd {
    command_id: AbiId,
    dma_addresses: VectorDmaAddresses,
    length: u32,
}

impl AbiVectorAdd {
    /// Creates a fixed-width vector-add command.
    pub const fn new(command_id: AbiId, dma_addresses: VectorDmaAddresses, length: u32) -> Self {
        Self {
            command_id,
            dma_addresses,
            length,
        }
    }

    /// Command identifier.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }

    /// Absolute first-source DMA address.
    pub const fn source_a(self) -> DmaAddress {
        self.dma_addresses.source_a
    }

    /// Absolute second-source DMA address.
    pub const fn source_b(self) -> DmaAddress {
        self.dma_addresses.source_b
    }

    /// Absolute destination DMA address.
    pub const fn destination(self) -> DmaAddress {
        self.dma_addresses.destination
    }

    /// Vector length in bytes.
    pub const fn length(self) -> u32 {
        self.length
    }
}
