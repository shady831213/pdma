use pdma_model_contract::abi::mmio::MmioAccessError;
use pdma_model_device::AsyncDeviceHandle;

/// Reads all three command-result latches through the public MMIO seam.
pub(super) async fn result_latches(
    handle: &AsyncDeviceHandle,
) -> Result<[Vec<u8>; 3], MmioAccessError> {
    Ok([
        handle.read_mmio(0x024, 4).await?,
        handle.read_mmio(0x16c, 4).await?,
        handle.read_mmio(0x224, 4).await?,
    ])
}
