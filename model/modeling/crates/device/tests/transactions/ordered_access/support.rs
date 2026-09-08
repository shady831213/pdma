use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_device::{AsyncDeviceHandle, MappedDmaRegion, MmioRegisterEndpoint};
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

pub(in crate::transactions) async fn bootstrap(
    handle: &AsyncDeviceHandle,
) -> TestResult<MappedDmaRegion> {
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    let submission = *submission;
    let regions = handle.commit_memory_stage(stage).await?;
    program_activation(&handle.register_endpoint(), &regions).await?;
    handle
        .register_endpoint()
        .write(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await?;
    Ok(submission)
}

pub(in crate::transactions) async fn cursors(handle: &AsyncDeviceHandle) -> TestResult<Vec<u64>> {
    let mut values = Vec::new();
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::SQ_CAPTURED.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::CQ_CONSUMER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
        QueueRegisters::EQ_CONSUMER.offset(),
    ] {
        values.push(read_cursor(handle, offset).await?);
    }
    Ok(values)
}

pub(in crate::transactions) async fn read_cursor(
    handle: &AsyncDeviceHandle,
    offset: usize,
) -> TestResult<u64> {
    let mut bytes = [0; 8];
    handle
        .register_endpoint()
        .read(queue_offset(0, offset)?, &mut bytes)
        .await?;
    Ok(u64::from_le_bytes(bytes))
}

pub(in crate::transactions) async fn publish(
    handle: &AsyncDeviceHandle,
    producer: u64,
) -> TestResult {
    handle
        .register_endpoint()
        .write(
            queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?,
            &producer.to_le_bytes(),
        )
        .await?;
    Ok(())
}

pub(in crate::transactions) async fn program_activation(
    endpoint: &MmioRegisterEndpoint,
    regions: &[MappedDmaRegion],
) -> TestResult {
    let [sq, cq, eq] = regions else {
        return Err("three rings required".into());
    };
    for (offset, value) in [
        (QueueRegisters::SQ_BASE.offset(), sq.base().raw()),
        (QueueRegisters::CQ_BASE.offset(), cq.base().raw()),
        (QueueRegisters::EQ_BASE.offset(), eq.base().raw()),
    ] {
        endpoint
            .write(queue_offset(0, offset)?, &value.to_le_bytes())
            .await?;
    }
    for offset in [
        QueueRegisters::SQ_DEPTH.offset(),
        QueueRegisters::CQ_DEPTH.offset(),
        QueueRegisters::EQ_DEPTH.offset(),
    ] {
        endpoint
            .write(queue_offset(0, offset)?, &2_u32.to_le_bytes())
            .await?;
    }
    Ok(())
}
