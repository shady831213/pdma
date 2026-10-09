use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::memory::MemoryIdentity;
use std::cell::{Cell, RefCell};

pub(super) const ADDRESS: MemoryAddress = MemoryAddress::new(0x1000);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Tag {
    Amber([u8; 3]),
    Indigo(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Sideband {
    pub identity: MemoryIdentity,
    pub tag: Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    Validate,
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Error {
    MissingPage(MemoryAddress),
    Backing(MemoryAccessError),
}

impl From<Error> for pdma_model_dma::MemoryFailure {
    fn from(error: Error) -> Self {
        match error {
            Error::MissingPage(address) => Self::PageFault { address },
            Error::Backing(_) => Self::Access,
        }
    }
}

pub(super) struct PlatformMemory {
    pub backing: SharedMemory,
    pub reports: RefCell<Vec<(Sideband, Operation)>>,
    pub fault: Cell<bool>,
}

impl PlatformMemory {
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let backing = SharedMemory::new();
        backing.add_regions(&[MemoryRegion::try_new(ADDRESS, 4, MemoryAlignment::ONE)?])?;
        Ok(Self {
            backing,
            reports: RefCell::new(Vec::new()),
            fault: Cell::new(false),
        })
    }

    fn record(&self, sideband: &Sideband, op: Operation) -> Result<(), Error> {
        self.reports.borrow_mut().push((sideband.clone(), op));
        if self.fault.get() {
            return Err(Error::MissingPage(ADDRESS));
        }
        Ok(())
    }
}

impl MemoryAccess<Sideband> for PlatformMemory {
    type Error = Error;

    fn validate_with_sideband(
        &self,
        sideband: &Sideband,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.record(sideband, Operation::Validate)?;
        self.backing
            .validate(address, alignment, length)
            .map_err(Error::Backing)
    }

    fn read_with_sideband(
        &self,
        sideband: &Sideband,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.record(sideband, Operation::Read)?;
        self.backing
            .read(address, alignment, bytes)
            .map_err(Error::Backing)
    }

    fn write_with_sideband(
        &self,
        sideband: &Sideband,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.record(sideband, Operation::Write)?;
        self.backing
            .write(address, alignment, bytes)
            .map_err(Error::Backing)
    }
}
