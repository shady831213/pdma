use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_dma::MemoryFailure;
use std::cell::Cell;

pub(super) const ADDRESS: MemoryAddress = MemoryAddress::new(0x1000);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BackendError {
    MissingPage { address: MemoryAddress },
    WrongIdentity,
    Backing(MemoryAccessError),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BackendError {}

impl From<BackendError> for MemoryFailure {
    fn from(error: BackendError) -> Self {
        match error {
            BackendError::MissingPage { address } => Self::PageFault { address },
            BackendError::WrongIdentity | BackendError::Backing(_) => Self::Access,
        }
    }
}

pub(super) struct PagedMemory<S> {
    pub backing: SharedMemory,
    pub present: Cell<bool>,
    pub calls: Cell<usize>,
    identity: S,
}

impl<S: PartialEq> PagedMemory<S> {
    pub(super) fn new(identity: S) -> Result<Self, Box<dyn std::error::Error>> {
        let backing = SharedMemory::new();
        backing.add_regions(&[MemoryRegion::try_new(ADDRESS, 4, MemoryAlignment::ONE)?])?;
        backing.write(ADDRESS, MemoryAlignment::ONE, &[13; 4])?;
        Ok(Self {
            backing,
            present: Cell::new(false),
            calls: Cell::new(0),
            identity,
        })
    }

    fn select(&self, identity: &S, address: MemoryAddress) -> Result<&SharedMemory, BackendError> {
        self.calls.set(self.calls.get() + 1);
        if identity != &self.identity {
            return Err(BackendError::WrongIdentity);
        }
        if !self.present.get() {
            return Err(BackendError::MissingPage { address });
        }
        Ok(&self.backing)
    }
}

impl<S: PartialEq> MemoryAccess<S> for PagedMemory<S> {
    type Error = BackendError;

    fn validate_with_sideband(
        &self,
        identity: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.select(identity, address)?
            .validate(address, alignment, length)
            .map_err(BackendError::Backing)
    }

    fn read_with_sideband(
        &self,
        identity: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.select(identity, address)?
            .read(address, alignment, bytes)
            .map_err(BackendError::Backing)
    }

    fn write_with_sideband(
        &self,
        identity: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.select(identity, address)?
            .write(address, alignment, bytes)
            .map_err(BackendError::Backing)
    }
}
