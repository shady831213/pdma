use super::{AsyncDeviceHandle, publish, wake};
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount, decode_read, decode_write};

impl AsyncDeviceHandle {
    /// Reads one exact byte-bearing MMIO transaction.
    pub async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        let register = decode_read(offset, width)?;
        let state = self.shared.state.lock().await;
        Ok(crate::abi::endpoint::prepare_read(&state.core, register, offset).response)
    }

    /// Applies one exact byte-bearing MMIO transaction.
    pub async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        let (register, value) = decode_write(offset, bytes)?;
        let generation = {
            let mut state = self.shared.state.lock().await;
            let changed = crate::abi::endpoint::prepare_write(register, value, offset, bytes)
                .commit(&mut state.core);
            if changed {
                wake(&mut state);
            }
            changed.then_some(state.generation)
        };
        if let Some(generation) = generation {
            publish(&self.shared, generation);
        }
        Ok(())
    }
}
