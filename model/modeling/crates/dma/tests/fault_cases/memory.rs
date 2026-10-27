use hardware_interface_memory::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
};
use pdma_model_dma::MemoryFailure;
use std::cell::{Cell, RefCell};

#[derive(Debug)]
pub(super) enum BackendError {
    Backing(MemoryAccessError),
    MissingPage { address: MemoryAddress },
}

impl From<MemoryAccessError> for BackendError {
    fn from(error: MemoryAccessError) -> Self {
        Self::Backing(error)
    }
}

impl From<BackendError> for MemoryFailure {
    fn from(error: BackendError) -> Self {
        match error {
            BackendError::Backing(error) => error.into(),
            BackendError::MissingPage { address } => Self::PageFault { address },
        }
    }
}

#[derive(Debug)]
pub(super) struct Paged {
    pub memory: SharedMemory,
    pub missing: Cell<Option<(u64, bool)>>,
    pub faults: Cell<u32>,
    pub accesses: Cell<usize>,
    pub reads: RefCell<Vec<u64>>,
}

impl Paged {
    fn fault(
        &self,
        address: MemoryAddress,
        length: usize,
        write: bool,
    ) -> Result<(), BackendError> {
        if let Some((missing, writes)) = self.missing.get()
            && writes == write
            && missing >= address.raw()
            && missing - address.raw()
                < u64::try_from(length).map_err(|_| MemoryAccessError::AddressOverflow {
                    address,
                    length: u64::MAX,
                })?
        {
            self.faults.set(self.faults.get() + 1);
            return Err(BackendError::MissingPage {
                address: missing.into(),
            });
        }
        Ok(())
    }
}

impl MemoryAccess for Paged {
    type Error = BackendError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), BackendError> {
        self.accesses.set(self.accesses.get() + 1);
        self.memory
            .validate(address, alignment, length)
            .map_err(Into::into)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        out: &mut [u8],
    ) -> Result<(), BackendError> {
        self.accesses.set(self.accesses.get() + 1);
        self.reads.borrow_mut().push(address.raw());
        self.fault(address, out.len(), false)?;
        self.memory
            .read(address, alignment, out)
            .map_err(Into::into)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        self.accesses.set(self.accesses.get() + 1);
        self.fault(address, bytes.len(), true)?;
        self.memory
            .write(address, alignment, bytes)
            .map_err(Into::into)
    }
}
