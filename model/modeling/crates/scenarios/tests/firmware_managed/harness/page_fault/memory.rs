use super::{ChannelTag, platform::TaggedRequest};
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAccessError, MemoryAddress,
    MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_contract::memory::MemoryIdentity;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

const PAGE_SIZE: u64 = 256;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Access reported by the platform memory backend.
pub enum FaultAccess {
    /// Payload read.
    Read,
    /// Payload write.
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Platform fault identity, independent of the DMA response protocol.
pub struct FaultReport {
    pub(crate) ch_tag: ChannelTag,
    /// Function and address space selected by the memory router.
    pub identity: MemoryIdentity,
    /// First missing address touched by the access.
    pub address: MemoryAddress,
    /// Access that encountered the missing mapping.
    pub access: FaultAccess,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::harness) enum PageError {
    Access(MemoryAccessError),
    PageFault { address: MemoryAddress },
    ReportDelivery,
    MissingSource,
}

impl std::fmt::Display for PageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Access(error) => error.fmt(f),
            Self::PageFault { address } => write!(f, "page fault at {:#x}", address.raw()),
            Self::ReportDelivery => f.write_str("platform fault report delivery failed"),
            Self::MissingSource => f.write_str("faulting request has no DMA source tag"),
        }
    }
}

impl std::error::Error for PageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Access(error) => Some(error),
            Self::PageFault { .. } | Self::ReportDelivery | Self::MissingSource => None,
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
            PageError::Access(_) | PageError::ReportDelivery | PageError::MissingSource => {
                Self::Access
            }
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
    pub reports: mpsc::Sender<FaultReport>,
}

impl Paged {
    fn access(
        &self,
        (address, length): (MemoryAddress, u64),
        (request, access): (&TaggedRequest, FaultAccess),
    ) -> Result<(), PageError> {
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
            let address = (page * PAGE_SIZE).max(address.raw()).into();
            drop(missing);
            self.reports
                .try_send(FaultReport {
                    identity: request.identity,
                    ch_tag: request.ch_tag.clone().ok_or(PageError::MissingSource)?,
                    address,
                    access,
                })
                .map_err(|_| PageError::ReportDelivery)?;
            return Err(PageError::PageFault { address });
        }
        drop(missing);
        Ok(())
    }
}

impl MemoryAccess<TaggedRequest> for Paged {
    type Error = PageError;
    fn atomic32_with_sideband(
        &self,
        request: &TaggedRequest,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.memory
            .validate(address, MemoryAlignment::ONE, 4)
            .map_err(|error| AtomicAccessError::Access(PageError::Access(error)))?;
        let access = match operation {
            AtomicOperation::Load(_) => FaultAccess::Read,
            AtomicOperation::Store { .. } | AtomicOperation::Update { .. } => FaultAccess::Write,
        };
        self.access((address, 4), (request, access))
            .map_err(AtomicAccessError::Access)?;
        self.memory
            .atomic32(address, operation)
            .map_err(|error| error.map_access(PageError::Access))
    }

    fn atomic64_with_sideband(
        &self,
        request: &TaggedRequest,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.memory
            .validate(address, MemoryAlignment::ONE, 8)
            .map_err(|error| AtomicAccessError::Access(PageError::Access(error)))?;
        let access = match operation {
            AtomicOperation::Load(_) => FaultAccess::Read,
            AtomicOperation::Store { .. } | AtomicOperation::Update { .. } => FaultAccess::Write,
        };
        self.access((address, 8), (request, access))
            .map_err(AtomicAccessError::Access)?;
        self.memory
            .atomic64(address, operation)
            .map_err(|error| error.map_access(PageError::Access))
    }

    fn validate_with_sideband(
        &self,
        _request: &TaggedRequest,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.memory
            .validate(address, alignment, length)
            .map_err(PageError::Access)
    }
    fn read_with_sideband(
        &self,
        request: &TaggedRequest,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        out: &mut [u8],
    ) -> Result<(), Self::Error> {
        let length = u64::try_from(out.len()).map_err(|_| MemoryAccessError::AddressOverflow {
            address,
            length: u64::MAX,
        })?;
        self.memory.validate(address, alignment, length)?;
        self.access((address, length), (request, FaultAccess::Read))?;
        self.memory
            .read(address, alignment, out)
            .map_err(PageError::Access)
    }
    fn write_with_sideband(
        &self,
        request: &TaggedRequest,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        let length =
            u64::try_from(bytes.len()).map_err(|_| MemoryAccessError::AddressOverflow {
                address,
                length: u64::MAX,
            })?;
        self.memory.validate(address, alignment, length)?;
        self.access((address, length), (request, FaultAccess::Write))?;
        self.memory
            .write(address, alignment, bytes)
            .map_err(PageError::Access)
    }
}
