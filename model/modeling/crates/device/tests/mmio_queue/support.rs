use pdma_model_contract::semantic::{DeviceEpoch, QueueId};
use pdma_model_device::AsyncDeviceHandle;
use pdma_model_device::MappedDmaRegion;

pub(crate) async fn write_u32(
    handle: &AsyncDeviceHandle,
    offset: u32,
    value: u32,
) -> Result<(), pdma_model_contract::abi::mmio::MmioAccessError> {
    handle.write_mmio(offset, &value.to_le_bytes()).await
}

pub(crate) async fn write_u64(
    handle: &AsyncDeviceHandle,
    offset: u32,
    value: u64,
) -> Result<(), pdma_model_contract::abi::mmio::MmioAccessError> {
    handle.write_mmio(offset, &value.to_le_bytes()).await
}

pub(crate) async fn read_u32(
    handle: &AsyncDeviceHandle,
    offset: u32,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let bytes = handle.read_mmio(offset, 4).await?;
    let array: [u8; 4] = bytes.try_into().map_err(|_| "four MMIO bytes required")?;
    Ok(u32::from_le_bytes(array))
}

pub(crate) async fn read_u64(
    handle: &AsyncDeviceHandle,
    offset: u32,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let bytes = handle.read_mmio(offset, 8).await?;
    let array: [u8; 8] = bytes.try_into().map_err(|_| "eight MMIO bytes required")?;
    Ok(u64::from_le_bytes(array))
}

pub(crate) async fn activate_raw(
    handle: &AsyncDeviceHandle,
    epoch: DeviceEpoch,
    queue_id: QueueId,
    regions: &[MappedDmaRegion],
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let [submission, completion, event] = regions else {
        return Err("three committed queue regions required".into());
    };
    program_activation(
        handle,
        RawActivation::depth_two(epoch, queue_id, [*submission, *completion, *event]),
    )
    .await?;
    issue_queue_command(handle, 1).await
}

#[derive(Clone, Copy)]
pub(crate) struct RawActivation {
    epoch: DeviceEpoch,
    queue_id: QueueId,
    regions: [MappedDmaRegion; 3],
}

impl RawActivation {
    pub(crate) const fn depth_two(
        epoch: DeviceEpoch,
        queue_id: QueueId,
        regions: [MappedDmaRegion; 3],
    ) -> Self {
        Self {
            epoch,
            queue_id,
            regions,
        }
    }
}

pub(crate) async fn program_activation(
    handle: &AsyncDeviceHandle,
    raw: RawActivation,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let [submission, completion, event] = raw.regions;
    for (offset, value) in [
        (0x100, raw.epoch.raw()),
        (0x108, u64::try_from(raw.queue_id.raw())?),
        (0x130, 7),
        (0x138, submission.base().raw()),
        (0x148, completion.base().raw()),
        (0x158, event.base().raw()),
    ] {
        write_u64(handle, offset, value).await?;
    }
    for (offset, value) in [(0x120, 2), (0x124, 2), (0x140, 2), (0x150, 2), (0x160, 2)] {
        write_u32(handle, offset, value).await?;
    }
    Ok(())
}

pub(crate) async fn issue_queue_command(
    handle: &AsyncDeviceHandle,
    command: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    write_u32(handle, 0x168, command).await?;
    Ok(())
}

pub(crate) async fn read_response(
    handle: &AsyncDeviceHandle,
) -> Result<[u64; 9], Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut response = [0; 9];
    for (slot, offset) in response.iter_mut().zip((0x188..=0x1c8).step_by(8)) {
        *slot = read_u64(handle, offset).await?;
    }
    Ok(response)
}
