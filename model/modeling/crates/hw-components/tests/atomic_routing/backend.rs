use super::*;
use hardware_interface_runtime::MemoryRegion;
use pdma_model_contract::memory::ContextAttributesAddressSpace;
use std::cell::RefCell;

pub(super) const ADDRESS: MemoryAddress = MemoryAddress::new(0x1000);

pub(super) struct Backing {
    pub spaces: Vec<(MemoryIdentity, SharedMemory)>,
    pub reports: RefCell<Vec<(MemoryIdentity, &'static str)>>,
}

impl Backing {
    pub(super) fn new() -> TestResult<Self> {
        let spaces = [
            (0, ContextAttributes::DEFAULT),
            (
                0,
                ContextAttributes {
                    address_space: ContextAttributesAddressSpace::Pasid,
                    pasid: 0,
                },
            ),
            (1, ContextAttributes::DEFAULT),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (function, attributes))| {
            let memory = SharedMemory::new();
            memory.add_regions(&[MemoryRegion::try_new(ADDRESS, 8, MemoryAlignment::ONE)?])?;
            memory.write(
                ADDRESS,
                MemoryAlignment::ONE,
                &(10 + u64::try_from(index)?).to_le_bytes(),
            )?;
            Ok((
                MemoryIdentity {
                    function: FunctionId::new(function),
                    attributes,
                },
                memory,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
        Ok(Self {
            spaces,
            reports: RefCell::new(Vec::new()),
        })
    }

    pub(super) fn values(&self) -> Result<Vec<u64>, MemoryAccessError> {
        self.spaces
            .iter()
            .map(|(_, memory)| {
                memory
                    .read_typed(ADDRESS, MemoryAlignment::ONE)
                    .map(u64::from_le_bytes)
            })
            .collect()
    }

    fn get(
        &self,
        identity: MemoryIdentity,
        address: MemoryAddress,
    ) -> Result<&SharedMemory, MemoryAccessError> {
        self.spaces
            .iter()
            .find(|(key, _)| *key == identity)
            .map(|(_, memory)| memory)
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}

impl MemoryAccess<(MemoryIdentity, &'static str)> for Backing {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        _: &(MemoryIdentity, &'static str),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: u64,
    ) -> Result<(), Self::Error> {
        Err(MemoryAccessError::Unmapped { address })
    }
    fn read_with_sideband(
        &self,
        _: &(MemoryIdentity, &'static str),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: &mut [u8],
    ) -> Result<(), Self::Error> {
        Err(MemoryAccessError::Unmapped { address })
    }
    fn write_with_sideband(
        &self,
        _: &(MemoryIdentity, &'static str),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: &[u8],
    ) -> Result<(), Self::Error> {
        Err(MemoryAccessError::Unmapped { address })
    }
    fn atomic32_with_sideband(
        &self,
        sideband: &(MemoryIdentity, &'static str),
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.reports.borrow_mut().push(*sideband);
        self.get(sideband.0, address)
            .map_err(AtomicAccessError::Access)?
            .atomic32(address, operation)
    }
    fn atomic64_with_sideband(
        &self,
        sideband: &(MemoryIdentity, &'static str),
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.reports.borrow_mut().push(*sideband);
        self.get(sideband.0, address)
            .map_err(AtomicAccessError::Access)?
            .atomic64(address, operation)
    }
}

impl MemoryAccess<MemoryIdentity> for Backing {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.validate_with_sideband(&(*identity, "control"), address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.read_with_sideband(&(*identity, "control"), address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.write_with_sideband(&(*identity, "control"), address, alignment, bytes)
    }
    fn atomic32_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.atomic32_with_sideband(&(*identity, "control"), address, operation)
    }
    fn atomic64_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.atomic64_with_sideband(&(*identity, "control"), address, operation)
    }
}
