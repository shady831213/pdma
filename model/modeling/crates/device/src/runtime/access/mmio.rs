use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount, decode_read, decode_write};

impl AsyncDeviceHandle {
    /// Reads one exact byte-bearing MMIO transaction.
    pub async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        let register = decode_read(offset, width)?;
        self.shared
            .backend
            .interact(|state| state.core.read_register(register))
            .await
            .ok_or(MmioAccessError::ReservedOffset { offset })
    }

    /// Applies one exact byte-bearing MMIO transaction.
    pub async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        let (register, value) = decode_write(offset, bytes)?;
        self.shared
            .backend
            .update(|state| {
                state.core.write_mmio_register(register, value).map_or(
                    (Err(MmioAccessError::ReservedOffset { offset }), false),
                    |changed| (Ok(()), changed),
                )
            })
            .await
    }
}
