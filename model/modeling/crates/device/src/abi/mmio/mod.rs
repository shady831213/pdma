//! Exact numeric MMIO command dispatch and register storage.

pub mod bank;
pub mod dispatch;
mod global;
mod irq;
pub mod queue;
pub mod reset;

pub(crate) use bank::RegisterBanks;

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
        let state = self.shared.state.lock().await;
        Ok(super::endpoint::prepare_read(&state, register, offset).response)
    }

    /// Applies one exact byte-bearing MMIO transaction.
    pub async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        let (register, value) = decode_write(offset, bytes)?;
        let generation = {
            let mut state = self.shared.state.lock().await;
            let previous = state.generation;
            super::endpoint::prepare_write(register, value, offset, bytes).commit(&mut state);
            (state.generation != previous).then_some(state.generation)
        };
        if let Some(generation) = generation {
            crate::runtime::publish(&self.shared, generation);
        }
        Ok(())
    }
}
