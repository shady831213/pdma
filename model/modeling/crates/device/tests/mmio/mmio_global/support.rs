use pdma_model_device::AsyncDeviceHandle;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) async fn read_u32(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u32> {
    let bytes = handle.read_mmio(offset, 4).await?;
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| std::io::Error::other("MMIO read width changed"))?;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) async fn read_u64(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u64> {
    let bytes = handle.read_mmio(offset, 8).await?;
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| std::io::Error::other("MMIO read width changed"))?;
    Ok(u64::from_le_bytes(bytes))
}

pub(super) async fn write_u32(handle: &AsyncDeviceHandle, offset: u32, value: u32) -> TestResult {
    Ok(handle.write_mmio(offset, &value.to_le_bytes()).await?)
}

pub(super) async fn write_u64(handle: &AsyncDeviceHandle, offset: u32, value: u64) -> TestResult {
    Ok(handle.write_mmio(offset, &value.to_le_bytes()).await?)
}

pub(super) async fn response(handle: &AsyncDeviceHandle) -> TestResult<[u64; 9]> {
    Ok([
        u64::from(read_u32(handle, 0x040).await?),
        u64::from(read_u32(handle, 0x044).await?),
        read_u64(handle, 0x048).await?,
        read_u64(handle, 0x050).await?,
        u64::from(read_u32(handle, 0x058).await?),
        u64::from(read_u32(handle, 0x05c).await?),
        u64::from(read_u32(handle, 0x060).await?),
        u64::from(read_u32(handle, 0x064).await?),
        u64::from(read_u32(handle, 0x068).await?),
    ])
}

pub(super) async fn result(handle: &AsyncDeviceHandle) -> TestResult<[u64; 4]> {
    Ok([
        u64::from(read_u32(handle, 0x024).await?),
        read_u64(handle, 0x028).await?,
        read_u64(handle, 0x030).await?,
        read_u64(handle, 0x038).await?,
    ])
}
