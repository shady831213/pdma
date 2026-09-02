use pdma_model_device::AsyncDeviceHandle;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) async fn write_u32(handle: &AsyncDeviceHandle, offset: u32, value: u32) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub(super) async fn write_u64(handle: &AsyncDeviceHandle, offset: u32, value: u64) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub(super) async fn read_u32(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u32> {
    let raw = handle.read_mmio(offset, 4).await?;
    let [lane0, lane1, lane2, lane3] = raw.as_slice() else {
        return Err("MMIO read did not return four bytes".into());
    };
    Ok(u32::from_le_bytes([*lane0, *lane1, *lane2, *lane3]))
}

pub(super) async fn read_u64(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u64> {
    let raw = handle.read_mmio(offset, 8).await?;
    let [lane0, lane1, lane2, lane3, lane4, lane5, lane6, lane7] = raw.as_slice() else {
        return Err("MMIO read did not return eight bytes".into());
    };
    Ok(u64::from_le_bytes([
        *lane0, *lane1, *lane2, *lane3, *lane4, *lane5, *lane6, *lane7,
    ]))
}

pub(super) async fn raw_reset(handle: &AsyncDeviceHandle, expected_epoch: u64) -> TestResult {
    write_u64(handle, 0x018, expected_epoch).await?;
    write_u32(handle, 0x020, 4).await
}
