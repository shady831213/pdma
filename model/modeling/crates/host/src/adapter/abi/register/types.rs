use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::semantic::{BufferId, IrqNotification};

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

/// One committed logical PDMA mapping.
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

/// Result of one host-owned sequential notification claim.
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
    },
}
