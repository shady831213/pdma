mod bank;
mod dispatch;
mod global;
mod irq;
mod queue;
pub(super) mod reset;

pub(super) use bank::RegisterBanks;

use super::AsyncDeviceHandle;
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
        Ok(dispatch::read(&state, register).encode())
    }

    /// Applies one exact byte-bearing MMIO transaction.
    pub async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        let (register, value) = decode_write(offset, bytes)?;
        let mut state = self.shared.state.lock().await;
        dispatch::write(&mut state, register, value);
        Ok(())
    }
}
