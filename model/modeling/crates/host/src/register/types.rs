use hardware_interface_runtime::{
    MemoryAddress, MemoryAlignment, MemoryRegion, RegisterObservation,
};
use pdma_model_contract::{BufferId, EpochQueue, IrqNotification, RingCursor};

/// One logical PDMA mapping request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PdmaRegionRequest {
    length: u64,
    alignment: MemoryAlignment,
}

impl PdmaRegionRequest {
    /// Requests one logical mapping of `length` bytes.
    pub const fn new(length: u64, alignment: MemoryAlignment) -> Self {
        Self { length, alignment }
    }

    /// Requested logical byte length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Required storage alignment.
    pub const fn alignment(self) -> MemoryAlignment {
        self.alignment
    }
}

/// One committed or proposed logical PDMA mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PdmaMappedRegion {
    object: BufferId,
    base: MemoryAddress,
    length: u64,
    alignment: MemoryAlignment,
    storage: Option<MemoryRegion>,
}

impl PdmaMappedRegion {
    /// Creates a checked logical mapping projection.
    pub fn try_new(
        object: BufferId,
        base: MemoryAddress,
        length: u64,
        alignment: MemoryAlignment,
        storage: Option<MemoryRegion>,
    ) -> Result<Self, super::PdmaProvisioningError> {
        match (length, storage) {
            (0, None) => Ok(Self {
                object,
                base,
                length,
                alignment,
                storage,
            }),
            (0, Some(_)) | (_, None) => Err(super::PdmaProvisioningError::InvalidProjection),
            (_, Some(region))
                if region.base() == base
                    && region.length() == length
                    && region.alignment() == alignment =>
            {
                Ok(Self {
                    object,
                    base,
                    length,
                    alignment,
                    storage,
                })
            }
            (_, Some(_)) => Err(super::PdmaProvisioningError::InvalidProjection),
        }
    }

    /// Semantic buffer identity.
    pub const fn object(self) -> BufferId {
        self.object
    }
    /// Logical mapping base.
    pub const fn base(self) -> MemoryAddress {
        self.base
    }
    /// Logical byte length, including zero.
    pub const fn length(self) -> u64 {
        self.length
    }
    /// Required and installed storage alignment.
    pub const fn alignment(self) -> MemoryAlignment {
        self.alignment
    }
    /// Exact non-empty runtime storage, absent for a zero-length mapping.
    pub const fn storage(self) -> Option<MemoryRegion> {
        self.storage
    }
}

/// Opaque typed proposal that can only be consumed by a closed batch.
pub trait PdmaPreparedProposal: Send {
    /// Mutation-free proposed mapping projections.
    fn regions(&self) -> &[PdmaMappedRegion];
    /// Allocator generation captured during preparation.
    fn generation(&self) -> u64;
}

/// One byte-opaque DMA write carried by a closed batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdmaDmaWrite {
    address: MemoryAddress,
    bytes: Vec<u8>,
}

impl PdmaDmaWrite {
    /// Creates a write at an exact mapped address.
    pub const fn new(address: MemoryAddress, bytes: Vec<u8>) -> Self {
        Self { address, bytes }
    }
    /// Target address.
    pub const fn address(&self) -> MemoryAddress {
        self.address
    }
    /// Opaque bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Consumes the write into its canonical address and bytes.
    pub fn into_parts(self) -> (MemoryAddress, Vec<u8>) {
        (self.address, self.bytes)
    }
}

/// The closed submission-producer batch request.
#[derive(Debug)]
pub struct PdmaSubmissionBatch<P> {
    queue: EpochQueue,
    producer: RingCursor,
    descriptor: PdmaDmaWrite,
    indirect: Option<(P, PdmaDmaWrite)>,
}

impl<P> PdmaSubmissionBatch<P> {
    /// Creates the fixed indirect/write/publish/response grammar.
    pub const fn new(
        queue: EpochQueue,
        producer: RingCursor,
        descriptor: PdmaDmaWrite,
        indirect: Option<(P, PdmaDmaWrite)>,
    ) -> Self {
        Self {
            queue,
            producer,
            descriptor,
            indirect,
        }
    }

    /// Consumes the closed request into its canonical fields.
    pub fn into_parts(
        self,
    ) -> (
        EpochQueue,
        RingCursor,
        PdmaDmaWrite,
        Option<(P, PdmaDmaWrite)>,
    ) {
        (self.queue, self.producer, self.descriptor, self.indirect)
    }
}

/// Result of one readiness-plus-compound exact notification claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdmaClaimResult {
    /// Readiness ended because the source shut down.
    Shutdown,
    /// Readiness raced with another claimant and the command returned Empty.
    Empty,
    /// One complete notification was consumed and captured.
    Notification {
        /// Claimed semantic notification.
        notification: IrqNotification,
        /// Exact command/result/response observations emitted in the commit poll.
        observations: Vec<RegisterObservation>,
    },
}
