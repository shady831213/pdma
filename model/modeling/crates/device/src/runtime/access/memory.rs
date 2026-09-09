use crate::runtime::AsyncDeviceHandle;
use hardware_interface_runtime::MemoryAccessError;
use pdma_model_contract::abi::DmaAddress;

impl AsyncDeviceHandle {
    /// Returns the clone-shared runtime memory capability owned by this device core.
    pub fn memory_access(&self) -> hardware_interface_runtime::SharedMemoryAccess {
        self.memory.clone()
    }

    /// Reads one checked address extent from the injected memory backend.
    pub async fn read_memory(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<Vec<u8>, MemoryAccessError> {
        self.shared
            .backend
            .interact(|_| crate::memory::read(&self.memory, address, length))
            .await
    }

    /// Writes one checked address extent and wakes pending device work on success.
    pub async fn write_memory(
        &self,
        address: DmaAddress,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.shared
            .backend
            .update(|_| {
                let result = crate::memory::write(&self.memory, address, bytes);
                let changed = result.is_ok();
                (result, changed)
            })
            .await
    }
}
