use crate::queue::hardware::{self, TestResult, read, write};
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, DmaAddress,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, MmioRegisterEndpoint};

pub(in crate::queue) struct Fixture {
    pub(in crate::queue) memory: hardware_interface_runtime::SharedMemoryAllocator,
    pub(in crate::queue) handle: AsyncDeviceHandle,
    pub(in crate::queue) endpoint: MmioRegisterEndpoint,
    worker: Option<AsyncDeviceWorker>,
    pub(in crate::queue) rings: [DmaAddress; 3],
}
impl Fixture {
    pub(in crate::queue) async fn new(resources: ResourceConfig) -> TestResult<Self> {
        let memory = hardware::allocator();
        let (device, worker) = AsyncDevice::new(memory.memory_access(), resources);
        let handle = device.handle();
        let rings = hardware::activate(&handle, &memory, 0, [4, 1, 1]).await?;
        let endpoint = handle.register_endpoint();
        handle.enable_tracing().await;
        Ok(Self {
            memory,
            handle,
            endpoint,
            worker: Some(worker),
            rings,
        })
    }
    pub(in crate::queue) fn start(
        &mut self,
    ) -> TestResult<tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>> {
        Ok(tokio::spawn(
            self.worker.take().ok_or("worker already started")?.run(),
        ))
    }
    pub(in crate::queue) async fn copy(&self, slot: u64, addresses: [DmaAddress; 2]) -> TestResult {
        let descriptor = AbiCommand::from_payload(
            slot + 10,
            AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                source_dma_address: addresses[0].raw(),
                destination_dma_address: addresses[1].raw(),
                length: 4.into(),
            }),
        );
        let mut bytes = [0; 64];
        descriptor.encode(&mut bytes)?;
        self.bytes(slot, &bytes).await
    }
    pub(in crate::queue) async fn bytes(&self, slot: u64, bytes: &[u8]) -> TestResult {
        self.handle
            .write_memory(self.rings[0].checked_add(slot * 64)?, bytes)
            .await?;
        self.cursor(QueueRegisters::SQ_PRODUCER.offset(), slot + 1)
            .await
    }
    pub(in crate::queue) async fn cursor(&self, offset: usize, value: u64) -> TestResult {
        write(&self.endpoint, queue_offset(0, offset)?, value).await
    }
    pub(in crate::queue) async fn read(&self, offset: usize) -> TestResult<u64> {
        read(&self.endpoint, queue_offset(0, offset)?).await
    }
}
pub(in crate::queue) async fn allocate_pair(fixture: &Fixture) -> TestResult<[DmaAddress; 2]> {
    let source = hardware::allocate(&fixture.memory, 4)?;
    let destination = hardware::allocate(&fixture.memory, 4)?;
    fixture
        .handle
        .write_memory(source.base(), &[1, 3, 5, 7])
        .await?;
    Ok([source.base(), destination.base()])
}
