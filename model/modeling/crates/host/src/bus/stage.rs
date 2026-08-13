use pdma_model_contract::{DmaAddress, PdmaMemoryRegion};

/// Opaque staging capability produced by an asynchronous device handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncDeviceBusStage(pub(super) pdma_model_device::PdmaMemoryStage);

/// Read-only capability for an uncommitted DMA allocation batch.
pub trait BusDmaStage: Send {
    /// Returns the proposed regions without exposing allocator state.
    fn proposed_regions(&self) -> &[PdmaMemoryRegion];
}

/// One byte-opaque write included in an atomic bus transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusDmaWrite {
    address: DmaAddress,
    bytes: Vec<u8>,
}

impl BusDmaWrite {
    /// Creates a write at one exact mapped address.
    pub const fn new(address: DmaAddress, bytes: Vec<u8>) -> Self {
        Self { address, bytes }
    }

    /// Target mapped address.
    pub const fn address(&self) -> DmaAddress {
        self.address
    }

    /// Opaque bytes written at acceptance.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn into_parts(self) -> (DmaAddress, Vec<u8>) {
        (self.address, self.bytes)
    }
}
