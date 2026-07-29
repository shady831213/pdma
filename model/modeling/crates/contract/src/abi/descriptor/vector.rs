use crate::abi::{AbiId, DmaAddress};

/// Source and destination IOVAs for vector addition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorIovas {
    source_a: DmaAddress,
    source_b: DmaAddress,
    destination: DmaAddress,
}

impl VectorIovas {
    /// Creates three absolute vector IOVAs.
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
    iovas: VectorIovas,
    length: u32,
}

impl AbiVectorAdd {
    /// Creates a fixed-width vector-add command.
    pub const fn new(command_id: AbiId, iovas: VectorIovas, length: u32) -> Self {
        Self {
            command_id,
            iovas,
            length,
        }
    }

    /// Command identifier.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }

    /// Absolute first-source IOVA.
    pub const fn source_a(self) -> DmaAddress {
        self.iovas.source_a
    }

    /// Absolute second-source IOVA.
    pub const fn source_b(self) -> DmaAddress {
        self.iovas.source_b
    }

    /// Absolute destination IOVA.
    pub const fn destination(self) -> DmaAddress {
        self.iovas.destination
    }

    /// Vector length in bytes.
    pub const fn length(self) -> u32 {
        self.length
    }
}
