use super::support::{Fixture, Result};
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAccessError, MemoryAddress,
    MemoryAlignment, SharedMemory,
};
use pdma_model_dma::MemoryFailure;
use std::cell::Cell;

#[derive(Clone, Copy)]
pub(super) enum Fault {
    None,
    Unsupported,
    Access,
    Page,
}

pub(super) struct FaultMemory {
    pub memory: SharedMemory,
    pub atomic_fault: Cell<Fault>,
    pub response_fault: Cell<bool>,
}

#[derive(Debug)]
pub(super) enum Error {
    Backing(MemoryAccessError),
    Page(MemoryAddress),
    Access,
}

impl From<Error> for MemoryFailure {
    fn from(error: Error) -> Self {
        match error {
            Error::Backing(error) => error.into(),
            Error::Page(address) => Self::PageFault { address },
            Error::Access => Self::Access,
        }
    }
}

pub(super) fn fixture(fault: Fault) -> Result<Fixture<FaultMemory>> {
    let memory = SharedMemory::new();
    Fixture::with_backend(
        memory.clone(),
        FaultMemory {
            memory,
            atomic_fault: Cell::new(fault),
            response_fault: Cell::new(false),
        },
    )
}

impl MemoryAccess for FaultMemory {
    type Error = Error;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> std::result::Result<(), Error> {
        self.memory
            .validate(address, alignment, length)
            .map_err(Error::Backing)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        out: &mut [u8],
    ) -> std::result::Result<(), Error> {
        self.memory
            .read(address, alignment, out)
            .map_err(Error::Backing)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> std::result::Result<(), Error> {
        if self.response_fault.get() && address.raw() == 0x2000 {
            return Err(Error::Access);
        }
        self.memory
            .write(address, alignment, bytes)
            .map_err(Error::Backing)
    }
    fn atomic64_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> std::result::Result<u64, AtomicAccessError<Error>> {
        match self.atomic_fault.get() {
            Fault::None => self
                .memory
                .atomic64(address, operation)
                .map_err(|e| e.map_access(Error::Backing)),
            Fault::Unsupported => Err(AtomicAccessError::Unsupported),
            Fault::Access => Err(AtomicAccessError::Access(Error::Access)),
            Fault::Page => Err(AtomicAccessError::Access(Error::Page(address))),
        }
    }
}
