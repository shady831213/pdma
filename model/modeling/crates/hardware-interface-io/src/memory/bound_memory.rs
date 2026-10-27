use crate::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};

/// Unit-sideband access to a borrowed backend with one owned metadata value.
#[derive(Debug)]
pub struct BoundMemory<'a, M: ?Sized, S> {
    memory: &'a M,
    sideband: S,
}

impl<'a, M: ?Sized, S> BoundMemory<'a, M, S> {
    /// Binds metadata for every access through this adapter.
    pub const fn new(memory: &'a M, sideband: S) -> Self {
        Self { memory, sideband }
    }
}

impl<M: MemoryAccess<S> + ?Sized, S> MemoryAccess for BoundMemory<'_, M, S> {
    type Error = M::Error;

    fn atomic32_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: crate::memory::AtomicOperation<u32>,
    ) -> Result<u32, crate::memory::AtomicAccessError<Self::Error>> {
        self.memory
            .atomic32_with_sideband(&self.sideband, address, operation)
    }

    fn atomic64_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: crate::memory::AtomicOperation<u64>,
    ) -> Result<u64, crate::memory::AtomicAccessError<Self::Error>> {
        self.memory
            .atomic64_with_sideband(&self.sideband, address, operation)
    }

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.memory
            .validate_with_sideband(&self.sideband, address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.memory
            .read_with_sideband(&self.sideband, address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        self.memory
            .write_with_sideband(&self.sideband, address, alignment, source)
    }
}
