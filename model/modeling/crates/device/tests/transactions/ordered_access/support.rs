use hardware_interface_runtime::{
    MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAllocator,
};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_device::{AsyncDeviceHandle, MmioRegisterEndpoint};
use std::sync::Arc;

mod observer;
use observer::{GatedObserver, RecordingObserver};

pub(in crate::transactions) type TestResult<T = ()> =
    Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[derive(Clone)]
pub(in crate::transactions) struct TestDevice {
    handle: AsyncDeviceHandle,
    pub(in crate::transactions) memory: Arc<SharedMemory>,
    pub(in crate::transactions) allocator: SharedMemoryAllocator,
}

impl std::ops::Deref for TestDevice {
    type Target = AsyncDeviceHandle;
    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl TestDevice {
    fn new(handle: AsyncDeviceHandle, memory: Arc<SharedMemory>) -> Self {
        let allocator =
            SharedMemoryAllocator::new(Arc::clone(&memory), MemoryAddress::new(0x1_0000));
        Self {
            handle,
            memory,
            allocator,
        }
    }

    pub(in crate::transactions) fn allocate(&self, length: u64) -> TestResult<DmaAddress> {
        Ok(DmaAddress::new(
            self.allocator
                .allocate(length, MemoryAlignment::try_new(64)?)?
                .raw(),
        ))
    }

    pub(in crate::transactions) fn rings(&self, lengths: [u64; 3]) -> TestResult<[DmaAddress; 3]> {
        let [sq, cq, eq] = lengths;
        Ok([self.allocate(sq)?, self.allocate(cq)?, self.allocate(eq)?])
    }
}

pub(in crate::transactions) fn new_handle() -> TestDevice {
    let memory = Arc::new(SharedMemory::new());
    let (device, _worker) = pdma_model_device::AsyncDevice::new(
        Arc::clone(&memory),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    TestDevice::new(device.handle(), memory)
}

pub(in crate::transactions) fn observed_handle() -> (TestDevice, Arc<RecordingObserver>) {
    let (handle, observer, _) = observed_handle_with_memory();
    (handle, observer)
}

pub(in crate::transactions) fn observed_handle_with_memory() -> (
    TestDevice,
    Arc<RecordingObserver>,
    Arc<hardware_interface_runtime::SharedMemory>,
) {
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (handle, observer) = observed_handle_for_memory(&memory);
    (handle, observer, memory)
}

pub(in crate::transactions) fn observed_handle_for_memory(
    memory: &Arc<hardware_interface_runtime::SharedMemory>,
) -> (TestDevice, Arc<RecordingObserver>) {
    let observer = Arc::new(RecordingObserver::default());
    let (device, _worker) = pdma_model_device::AsyncDevice::new_observed(
        Arc::clone(memory),
        pdma_model_contract::semantic::ResourceConfig::default(),
        observer.clone(),
    );
    (
        TestDevice::new(device.handle(), Arc::clone(memory)),
        observer,
    )
}

pub(in crate::transactions) fn gated_handle() -> (
    TestDevice,
    Arc<GatedObserver>,
    Arc<hardware_interface_runtime::SharedMemory>,
) {
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(GatedObserver::new());
    let (device, _worker) = pdma_model_device::AsyncDevice::new_observed(
        Arc::clone(&memory),
        pdma_model_contract::semantic::ResourceConfig::default(),
        observer.clone(),
    );
    (
        TestDevice::new(device.handle(), Arc::clone(&memory)),
        observer,
        memory,
    )
}

pub(in crate::transactions) async fn bootstrap(handle: &TestDevice) -> TestResult<DmaAddress> {
    let regions = handle.rings([128, 64, 128])?;
    let submission = regions[0];
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
    regions: &[DmaAddress],
) -> TestResult {
    let [sq, cq, eq] = regions else {
        return Err("three rings required".into());
    };
    for (offset, value) in [
        (QueueRegisters::SQ_BASE.offset(), sq.raw()),
        (QueueRegisters::CQ_BASE.offset(), cq.raw()),
        (QueueRegisters::EQ_BASE.offset(), eq.raw()),
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
