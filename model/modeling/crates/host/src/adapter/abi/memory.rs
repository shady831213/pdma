use super::{PdmaMappedRegion, PdmaProvisioningCause, PdmaProvisioningError, PdmaRegionRequest};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryAllocationError,
    MemoryError, MemoryRegion, MemoryRegionError, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator,
};
use pdma_model_contract::semantic::BufferId;
use std::sync::{Arc, Mutex};

/// Software-owned buffer identities over independently provisioned runtime memory.
#[derive(Clone, Debug)]
pub struct PdmaHostMemory {
    allocator: SharedMemoryAllocator,
    identities: Arc<Mutex<Identities>>,
}

#[derive(Debug, Default)]
struct Identities {
    next: usize,
    empty: Vec<MemoryAddress>,
}

impl PdmaHostMemory {
    /// Uses shared runtime storage with allocations starting at DMA address 0x10000.
    pub fn new(memory: Arc<SharedMemory>) -> Self {
        Self {
            allocator: SharedMemoryAllocator::new(memory, MemoryAddress::new(0x1_0000)),
            identities: Arc::new(Mutex::new(Identities::default())),
        }
    }

    /// Returns address access without software identities or provisioning authority.
    pub fn memory_access(&self) -> SharedMemoryAccess {
        self.allocator.memory_access()
    }

    /// Commits one runtime allocation and its software identity synchronously.
    pub fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, PdmaProvisioningError> {
        let mut identities = match self.identities.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let next = identities
            .next
            .checked_add(1)
            .ok_or(PdmaProvisioningError::Provisioning(
                PdmaProvisioningCause::AddressExhausted,
            ))?;
        if request.length() == 0 {
            identities.empty.try_reserve(1).map_err(|_| {
                PdmaProvisioningError::Provisioning(PdmaProvisioningCause::StorageExhausted)
            })?;
        }
        let base = self
            .allocator
            .allocate(request.length(), request.alignment())
            .map_err(|error| map_allocation(&error))?;
        let storage = if request.length() == 0 {
            None
        } else {
            Some(
                MemoryRegion::try_new(base, request.length(), request.alignment())
                    .map_err(|_| PdmaProvisioningError::InvalidProjection)?,
            )
        };
        let region = PdmaMappedRegion::try_new(
            BufferId::new(identities.next),
            base,
            request.length(),
            request.alignment(),
            storage,
        )?;
        if region.length() == 0 {
            identities.empty.push(base);
        }
        identities.next = next;
        drop(identities);
        Ok(region)
    }

    fn is_empty(&self, address: MemoryAddress, length: u64) -> bool {
        if length != 0 {
            return false;
        }
        let identities = match self.identities.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        identities.empty.contains(&address)
    }
}

impl MemoryAccess for PdmaHostMemory {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        if self.is_empty(address, length) && address.raw() % alignment.raw() == 0 {
            return Ok(());
        }
        self.memory_access().validate(address, alignment, length)
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        if destination.is_empty()
            && self.is_empty(address, 0)
            && address.raw() % alignment.raw() == 0
        {
            return Ok(());
        }
        self.memory_access().read(address, alignment, destination)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        if source.is_empty() && self.is_empty(address, 0) && address.raw() % alignment.raw() == 0 {
            return Ok(());
        }
        self.memory_access().write(address, alignment, source)
    }
}

const fn map_allocation(error: &MemoryAllocationError) -> PdmaProvisioningError {
    let cause = match error {
        MemoryAllocationError::AddressExhausted => PdmaProvisioningCause::AddressExhausted,
        MemoryAllocationError::Region(MemoryRegionError::AddressOverflow { .. }) => {
            PdmaProvisioningCause::AddressExhausted
        }
        MemoryAllocationError::Region(MemoryRegionError::StorageSizeOutOfRange { .. }) => {
            PdmaProvisioningCause::StorageExhausted
        }
        MemoryAllocationError::Storage(
            MemoryError::StorageExhausted { .. } | MemoryError::RegionTableExhausted { .. },
        ) => PdmaProvisioningCause::StorageExhausted,
        MemoryAllocationError::Storage(MemoryError::Overlap { .. }) => {
            PdmaProvisioningCause::OverlappingRegion
        }
        MemoryAllocationError::Storage(
            MemoryError::StaleRevision { .. } | MemoryError::RevisionExhausted,
        )
        | MemoryAllocationError::Region(
            MemoryRegionError::Empty | MemoryRegionError::MisalignedBase { .. },
        ) => PdmaProvisioningCause::InvalidRegion,
    };
    PdmaProvisioningError::Provisioning(cause)
}
