use crate::dma::PdmaMemoryStage;
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::{EpochQueue, RingCursor};

/// One byte-opaque DMA write committed by a bus transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaWrite {
    address: DmaAddress,
    bytes: Vec<u8>,
}

impl DmaWrite {
    /// Creates a write at an exact mapped address.
    pub const fn new(address: DmaAddress, bytes: Vec<u8>) -> Self {
        Self { address, bytes }
    }

    pub(crate) const fn address(&self) -> DmaAddress {
        self.address
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One all-or-none host submission acceptance request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionAcceptance {
    pub(crate) queue: EpochQueue,
    pub(crate) expected_producer: RingCursor,
    pub(crate) descriptor: DmaWrite,
    pub(crate) indirect: Option<(PdmaMemoryStage, DmaWrite)>,
}

impl SubmissionAcceptance {
    /// Creates an acceptance request from already prepared opaque bytes.
    pub const fn new(
        queue: EpochQueue,
        expected_producer: RingCursor,
        descriptor: DmaWrite,
        indirect: Option<(PdmaMemoryStage, DmaWrite)>,
    ) -> Self {
        Self {
            queue,
            expected_producer,
            descriptor,
            indirect,
        }
    }
}
