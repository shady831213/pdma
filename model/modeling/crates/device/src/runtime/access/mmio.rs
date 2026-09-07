use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount, decode_read, decode_write};

impl AsyncDeviceHandle {
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn __inspect_register_image_for_test<R: Send>(
        &self,
        operation: impl for<'view> FnOnce(pdma_model_contract::abi::mmio::PdmaMmioMapView<'view>) -> R
        + Send,
    ) -> R {
        self.shared
            .backend
            .interact(|state| state.core.registers.inspect_for_test(operation))
            .await
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub async fn __mutate_register_image_for_test<R: Send>(
        &self,
        operation: impl for<'view> FnOnce(
            pdma_model_contract::abi::mmio::PdmaMmioMapViewMut<'view>,
        ) -> R
        + Send,
    ) -> R {
        self.shared
            .backend
            .interact(|state| state.core.registers.mutate_for_test(operation))
            .await
    }

    /// Reads one exact byte-bearing MMIO transaction.
    pub async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        let register = decode_read(offset, width)?;
        Ok(self
            .shared
            .backend
            .interact(|state| {
                crate::interface::mmio::prepare_read(&state.core, register, offset).response
            })
            .await)
    }

    /// Applies one exact byte-bearing MMIO transaction.
    pub async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        let (register, value) = decode_write(offset, bytes)?;
        self.shared
            .backend
            .update(|state| {
                let changed = crate::interface::mmio::prepare_write(register, value, offset, bytes)
                    .commit(&mut state.core);
                ((), changed)
            })
            .await;
        Ok(())
    }
}
