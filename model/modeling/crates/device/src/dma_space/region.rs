use pdma_model_contract::abi::{AbiAddressError, DmaAddress, DmaRegion};
use pdma_model_contract::semantic::BufferId;

/// Device-owned association between one semantic DMA object and its exact ABI extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedDmaRegion {
    object: BufferId,
    region: DmaRegion,
}

impl MappedDmaRegion {
    pub(super) const fn new(object: BufferId, region: DmaRegion) -> Self {
        Self { object, region }
    }

    /// Creates a device mapping whose exact DMA extent is representable.
    pub const fn try_new(
        object: BufferId,
        base: DmaAddress,
        length: u64,
    ) -> Result<Self, AbiAddressError> {
        match DmaRegion::try_new(base, length) {
            Ok(region) => Ok(Self::new(object, region)),
            Err(error) => Err(error),
        }
    }

    /// Semantic DMA object identity.
    pub const fn object(self) -> BufferId {
        self.object
    }

    /// Exact base DMA address.
    pub const fn base(self) -> DmaAddress {
        self.region.base()
    }

    /// Exact fixed-width DMA extent.
    pub const fn region(self) -> DmaRegion {
        self.region
    }

    /// Fixed-width byte length.
    pub const fn length(self) -> u64 {
        self.region.length()
    }

    /// Checked exclusive end address.
    pub const fn exclusive_end(self) -> DmaAddress {
        self.region.exclusive_end()
    }

    /// Whether the mapping contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.region.is_empty()
    }
}
