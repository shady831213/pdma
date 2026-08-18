use pdma_model_contract::semantic::{
    CommandId, CopyCommand, CopyRequest, EventRouting, ExternalCommand, QueueConfig,
    QueueConfigHandle, SoftwareOwnerId,
};
use pdma_model_device::AsyncDeviceHandle;

pub const RESULT: u32 = 0x224;
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub async fn create_queue(
    handle: &AsyncDeviceHandle,
    depth: usize,
) -> TestResult<QueueConfigHandle> {
    Ok(handle
        .create_queue(QueueConfig::new(
            depth,
            depth,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await?)
}

pub async fn select_completion(handle: &AsyncDeviceHandle, queue: QueueConfigHandle) -> TestResult {
    write_u64(handle, 0x200, queue.epoch().raw()).await?;
    write_u64(
        handle,
        0x208,
        u64::try_from(queue.completion_ring_id().raw())?,
    )
    .await?;
    write_u32(handle, 0x210, 0).await?;
    Ok(())
}

pub async fn write_u32(handle: &AsyncDeviceHandle, offset: u32, value: u32) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub async fn write_u64(handle: &AsyncDeviceHandle, offset: u32, value: u64) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub async fn read_u32(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u32> {
    let raw = handle.read_mmio(offset, 4).await?;
    let [lane0, lane1, lane2, lane3] = raw.as_slice() else {
        return Err("MMIO read did not return four bytes".into());
    };
    Ok(u32::from_le_bytes([*lane0, *lane1, *lane2, *lane3]))
}

pub async fn read_u64(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u64> {
    let raw = handle.read_mmio(offset, 8).await?;
    let [lane0, lane1, lane2, lane3, lane4, lane5, lane6, lane7] = raw.as_slice() else {
        return Err("MMIO read did not return eight bytes".into());
    };
    Ok(u64::from_le_bytes([
        *lane0, *lane1, *lane2, *lane3, *lane4, *lane5, *lane6, *lane7,
    ]))
}

pub async fn response(handle: &AsyncDeviceHandle) -> TestResult<[u64; 9]> {
    Ok([
        read_u64(handle, 0x240).await?,
        read_u64(handle, 0x248).await?,
        u64::from(read_u32(handle, 0x250).await?),
        u64::from(read_u32(handle, 0x254).await?),
        u64::from(read_u32(handle, 0x258).await?),
        u64::from(read_u32(handle, 0x25c).await?),
        read_u64(handle, 0x260).await?,
        read_u64(handle, 0x268).await?,
        read_u64(handle, 0x270).await?,
    ])
}

pub async fn submit_copy(
    handle: &AsyncDeviceHandle,
    queue: QueueConfigHandle,
    command: usize,
) -> TestResult {
    let source = handle.allocate_buffer(1).await?;
    let destination = handle.allocate_buffer(1).await?;
    handle
        .submit_command(
            queue,
            ExternalCommand::Copy(CopyCommand::new(
                CommandId::new(command),
                queue.queue_id(),
                queue.completion_ring_id(),
                CopyRequest::new(source, destination, 1),
            )),
        )
        .await?;
    Ok(())
}
