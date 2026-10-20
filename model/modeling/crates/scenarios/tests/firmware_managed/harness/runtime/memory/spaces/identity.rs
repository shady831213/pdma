use super::{FunctionSpaces, SpaceError};
use crate::harness::runtime::platform::MemoryRequest;
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use pdma_model_contract::memory::MemoryIdentity;

impl<M: MemoryAccess> MemoryAccess<MemoryIdentity> for FunctionSpaces<M> {
    type Error = SpaceError<M::Error>;
    fn atomic32_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.atomic32_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            operation,
        )
    }

    fn atomic64_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.atomic64_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            operation,
        )
    }

    fn validate_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.validate_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            alignment,
            length,
        )
    }

    fn read_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.read_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            alignment,
            bytes,
        )
    }

    fn write_with_sideband(
        &self,
        identity: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.write_with_sideband(
            &MemoryRequest {
                channel: None,
                identity: *identity,
            },
            address,
            alignment,
            bytes,
        )
    }
}
