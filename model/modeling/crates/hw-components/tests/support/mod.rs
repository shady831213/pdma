use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_io::memory::{MemoryAccessError, MemoryRegion, SharedMemory};
use pdma_model_contract::memory::{
    ContextAttributes, ContextAttributesAddressSpace, FunctionId, MemoryIdentity,
};

/// The same mapped address in each independent space.
pub const ADDRESS: MemoryAddress = MemoryAddress::new(0x1000);
/// Independent memory backings keyed by the resolved hardware identity.
#[derive(Debug)]
pub struct Spaces(pub Vec<(MemoryIdentity, SharedMemory)>);
impl Spaces {
    /// Maps distinct bytes at ADDRESS for Default, PASID zero and another Function.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut spaces = Vec::new();
        for (function, attributes, byte) in [
            (0, ContextAttributes::DEFAULT, 11),
            (
                0,
                ContextAttributes {
                    address_space: ContextAttributesAddressSpace::Pasid,
                    pasid: 0,
                },
                22,
            ),
            (1, ContextAttributes::DEFAULT, 33),
        ] {
            let memory = SharedMemory::new();
            memory.add_regions(&[MemoryRegion::try_new(ADDRESS, 4, MemoryAlignment::ONE)?])?;
            memory.write(ADDRESS, MemoryAlignment::ONE, &[byte; 4])?;
            spaces.push((
                MemoryIdentity {
                    function: FunctionId::new(function),
                    attributes,
                },
                memory,
            ));
        }
        Ok(Self(spaces))
    }
    fn get(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
    ) -> Result<&SharedMemory, MemoryAccessError> {
        self.0
            .iter()
            .find(|(key, _)| key == identity)
            .map(|(_, memory)| memory)
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}
impl MemoryAccess<MemoryIdentity> for Spaces {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.get(identity, address)?
            .validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(identity, address)?.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(identity, address)?
            .write(address, alignment, bytes)
    }
}
