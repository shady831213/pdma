use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::mmio::{MmioByteCount, QueueResponseBank};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochQueue, EventRouting, QueueActivation, QueueConfig, QueueControl, QueueId,
    QueueRingObjects, RingCursor, SoftwareOwnerId,
};
use pdma_model_device::{AsyncDeviceHandle, MappedDmaRegion, PdmaMemoryStage};
use std::sync::Arc;

mod observer;
use observer::{GatedObserver, RecordingObserver};

pub(in crate::transactions) type TestResult<T = ()> =
    Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub(in crate::transactions) fn new_handle() -> AsyncDeviceHandle {
    let (device, _worker) = pdma_model_device::AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    device.handle()
}

pub(in crate::transactions) fn observed_handle() -> (AsyncDeviceHandle, Arc<RecordingObserver>) {
    let (handle, observer, _) = observed_handle_with_memory();
    (handle, observer)
}

pub(in crate::transactions) fn observed_handle_with_memory() -> (
    AsyncDeviceHandle,
    Arc<RecordingObserver>,
    Arc<hardware_interface_runtime::SharedMemory>,
) {
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (handle, observer) = observed_handle_for_memory(&memory);
    (handle, observer, memory)
}

pub(in crate::transactions) fn observed_handle_for_memory(
    memory: &Arc<hardware_interface_runtime::SharedMemory>,
) -> (AsyncDeviceHandle, Arc<RecordingObserver>) {
    let observer = Arc::new(RecordingObserver::default());
    let (device, _worker) = pdma_model_device::AsyncDevice::new_observed(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::clone(memory)),
        pdma_model_contract::semantic::ResourceConfig::default(),
        observer.clone(),
    );
    (device.handle(), observer)
}

pub(in crate::transactions) fn gated_handle() -> (
    AsyncDeviceHandle,
    Arc<GatedObserver>,
    Arc<hardware_interface_runtime::SharedMemory>,
) {
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(GatedObserver::new());
    let (device, _worker) = pdma_model_device::AsyncDevice::new_observed(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::clone(&memory)),
        pdma_model_contract::semantic::ResourceConfig::default(),
        observer.clone(),
    );
    (device.handle(), observer, memory)
}

pub(in crate::transactions) fn activation(
    stage: &PdmaMemoryStage,
    epoch: DeviceEpoch,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission, completion, event] = stage.regions() else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::try_from(stage.regions().len()).unwrap_or(u32::MAX),
            },
        );
    };
    Ok(QueueActivation::new(
        epoch,
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    ))
}

pub(in crate::transactions) async fn bootstrap(
    handle: &AsyncDeviceHandle,
) -> TestResult<(EpochQueue, MappedDmaRegion, Vec<u8>)> {
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    let submission = *submission;
    let activation = activation(&stage, DeviceEpoch::initial())?;
    let regions = handle.commit_memory_stage(stage).await?;
    program_activation(handle, &regions).await?;
    handle
        .register_endpoint()
        .write(0x164, &1_u32.to_le_bytes())
        .await?;
    let response = response_bytes(handle).await?;
    Ok((activation.queue(), submission, response))
}

pub(in crate::transactions) fn response(
    status: pdma_model_device::QueueStatus,
) -> QueueResponseBank {
    QueueResponseBank {
        epoch: status.queue().epoch().raw().into(),
        queue_id: u64::try_from(status.queue().queue_id().raw())
            .unwrap_or(u64::MAX)
            .into(),
        sq_producer: status.submission().producer().raw().into(),
        sq_captured: status.captured_submission_producer().raw().into(),
        sq_consumer: status.submission().consumer().raw().into(),
        cq_producer: status.completion().producer().raw().into(),
        cq_consumer: status.completion().consumer().raw().into(),
        eq_producer: status.event().producer().raw().into(),
        eq_consumer: status.event().consumer().raw().into(),
    }
}

pub(in crate::transactions) async fn queue_latches(
    handle: &AsyncDeviceHandle,
) -> TestResult<Vec<Vec<u8>>> {
    let mut values = Vec::new();
    for (offset, width) in [
        (0x168, MmioByteCount::from(4_u8)),
        (0x170, MmioByteCount::from(8_u8)),
        (0x178, MmioByteCount::from(8_u8)),
        (0x180, MmioByteCount::from(8_u8)),
        (0x188, MmioByteCount::from(8_u8)),
    ] {
        values.push(handle.read_mmio(offset, width).await?);
    }
    Ok(values)
}

pub(in crate::transactions) async fn queue_status(
    handle: &AsyncDeviceHandle,
    queue: EpochQueue,
) -> TestResult<pdma_model_device::QueueStatus> {
    Ok(handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?)
}

pub(in crate::transactions) async fn publish(
    handle: &AsyncDeviceHandle,
    queue: EpochQueue,
    expected: RingCursor,
) -> TestResult {
    for (offset, value) in [
        (0x100, queue.epoch().raw()),
        (0x108, u64::try_from(queue.queue_id().raw())?),
        (0x110, expected.raw()),
        (0x118, expected.raw() + 1),
    ] {
        handle
            .register_endpoint()
            .write(offset, &value.to_le_bytes())
            .await?;
    }
    handle
        .register_endpoint()
        .write(0x164, &3_u32.to_le_bytes())
        .await?;
    Ok(())
}

pub(in crate::transactions) async fn program_activation(
    handle: &AsyncDeviceHandle,
    regions: &[MappedDmaRegion],
) -> TestResult {
    let [sq, cq, eq] = regions else {
        return Err("three rings required".into());
    };
    for (offset, value) in [
        (0x100, 0_u64),
        (0x108, 0),
        (0x130, 7),
        (0x138, sq.base().raw()),
        (0x148, cq.base().raw()),
        (0x158, eq.base().raw()),
    ] {
        handle
            .register_endpoint()
            .write(offset, &value.to_le_bytes())
            .await?;
    }
    for offset in [0x120, 0x124, 0x140, 0x150, 0x160] {
        handle
            .register_endpoint()
            .write(offset, &2_u32.to_le_bytes())
            .await?;
    }
    Ok(())
}

pub(in crate::transactions) async fn response_bytes(
    handle: &AsyncDeviceHandle,
) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    for offset in (0x188..=0x1c8).step_by(8) {
        let mut value = [0; 8];
        handle.register_endpoint().read(offset, &mut value).await?;
        bytes.extend(value);
    }
    Ok(bytes)
}
