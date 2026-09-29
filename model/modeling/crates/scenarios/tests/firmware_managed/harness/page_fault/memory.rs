use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

const PAGE_SIZE: u64 = 256;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::harness) enum PageError {
    Access(MemoryAccessError),
    PageFault { address: MemoryAddress },
}

impl std::fmt::Display for PageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Access(error) => error.fmt(f),
            Self::PageFault { address } => write!(f, "page fault at {:#x}", address.raw()),
        }
    }
}

impl std::error::Error for PageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Access(error) => Some(error),
            Self::PageFault { .. } => None,
        }
    }
}

impl From<MemoryAccessError> for PageError {
    fn from(error: MemoryAccessError) -> Self {
        Self::Access(error)
    }
}

impl From<PageError> for pdma_model_dma::MemoryFailure {
    fn from(error: PageError) -> Self {
        match error {
            PageError::Access(_) => Self::Access,
            PageError::PageFault { address } => Self::PageFault { address },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::harness) struct PageTable(Arc<Mutex<BTreeSet<u64>>>);

impl PageTable {
    pub(in crate::harness) fn evict(&self, address: MemoryAddress) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(address.raw() / PAGE_SIZE);
    }

    pub(in crate::harness) fn repair(&self, address: MemoryAddress) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&(address.raw() / PAGE_SIZE))
    }
}

#[derive(Debug)]
pub(super) struct Paged {
    pub memory: SharedMemoryAccess,
    pub pages: PageTable,
}

impl Paged {
    fn access<T>(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
        action: impl FnOnce() -> Result<T, MemoryAccessError>,
    ) -> Result<T, PageError> {
        self.memory.validate(address, alignment, length)?;
        let missing = self
            .pages
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if length != 0
            && let Some(page) = missing
                .range(address.raw() / PAGE_SIZE..=(address.raw() + length - 1) / PAGE_SIZE)
                .next()
        {
            return Err(PageError::PageFault {
                address: (page * PAGE_SIZE).max(address.raw()).into(),
            });
        }
        let result = action();
        drop(missing);
        result.map_err(PageError::Access)
    }
}

impl MemoryAccess for Paged {
    type Error = PageError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.access(address, alignment, length, || Ok(()))
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        out: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.access(
            address,
            alignment,
            u64::try_from(out.len()).map_err(|_| MemoryAccessError::AddressOverflow {
                address,
                length: u64::MAX,
            })?,
            || self.memory.read(address, alignment, out),
        )
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.access(
            address,
            alignment,
            u64::try_from(bytes.len()).map_err(|_| MemoryAccessError::AddressOverflow {
                address,
                length: u64::MAX,
            })?,
            || self.memory.write(address, alignment, bytes),
        )
    }
}
