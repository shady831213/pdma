mod bank;
pub(in crate::runner) mod dispatch;
mod global;
mod irq;
pub(in crate::runner) mod queue;
pub(in crate::runner) mod reset;

pub(in crate::runner) use bank::RegisterBanks;

use crate::runner::AsyncDeviceHandle;
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
        Ok(super::register_endpoint::prepare_read(&state, register, offset).response)
    }

    /// Applies one exact byte-bearing MMIO transaction.
    pub async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        let (register, value) = decode_write(offset, bytes)?;
        let generation = {
            let mut state = self.shared.state.lock().await;
            let previous = state.generation;
            super::register_endpoint::prepare_write(register, value, offset, bytes)
                .commit(&mut state);
            (state.generation != previous).then_some(state.generation)
        };
        if let Some(generation) = generation {
            crate::runner::publish(&self.shared, generation);
        }
        Ok(())
    }
}
