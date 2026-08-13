use super::super::{AsyncDeviceBusStage, BusDmaStage, BusSubmission};
use crate::ModelResult;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount};
use pdma_model_contract::{DmaAddress, PdmaMemoryRegion, QueueActivation};
use pdma_model_device::{MmioBatchError, PdmaMemoryError, QueueResponseBytes};

/// Device-facing operations used by the MMIO bus and literal recording transports.
pub trait MmioDeviceTransport: Clone + Send + Sync {
    /// Transport-owned checked DMA staging capability.
    type Stage: BusDmaStage;

    /// Allocates one checked committed DMA region.
    fn allocate_memory_region(
        &self,
        len: usize,
    ) -> impl Future<Output = ModelResult<PdmaMemoryRegion>> + Send;
    /// Proposes checked DMA mappings.
    fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> impl Future<Output = Result<Self::Stage, PdmaMemoryError>> + Send;
    /// Reads checked DMA bytes.
    fn read_dma(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> impl Future<Output = Result<Vec<u8>, PdmaMemoryError>> + Send;
    /// Writes checked DMA bytes.
    fn write_dma(
        &self,
        address: DmaAddress,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), PdmaMemoryError>> + Send;
    /// Executes the exact closed bootstrap batch.
    fn bootstrap_batch(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> impl Future<Output = Result<QueueResponseBytes, MmioBatchError>> + Send;
    /// Executes the exact closed submission batch.
    fn submission_batch(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> impl Future<Output = Result<QueueResponseBytes, MmioBatchError>> + Send;
    /// Performs one byte-bearing MMIO read.
    fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> impl Future<Output = Result<Vec<u8>, MmioAccessError>> + Send;
    /// Performs one byte-bearing MMIO write.
    fn write_mmio(
        &self,
        offset: u32,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), MmioAccessError>> + Send;
    /// Waits until a notification can be claimed without consuming it.
    fn wait_irq_notification_ready(&self) -> impl Future<Output = bool> + Send;
}

impl MmioDeviceTransport for pdma_model_device::AsyncDeviceHandle {
    type Stage = AsyncDeviceBusStage;

    async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        self.allocate_memory_region(len).await.map_err(Into::into)
    }
    async fn stage_memory_regions(&self, lengths: &[u64]) -> Result<Self::Stage, PdmaMemoryError> {
        self.stage_memory_regions(lengths)
            .await
            .map(AsyncDeviceBusStage)
    }
    async fn read_dma(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, PdmaMemoryError> {
        self.read_memory(address, length).await
    }
    async fn write_dma(&self, address: DmaAddress, bytes: &[u8]) -> Result<(), PdmaMemoryError> {
        self.write_memory(address, bytes).await
    }
    async fn bootstrap_batch(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.bootstrap_queue_batch(pdma_model_device::QueueBootstrapBatch::new(
            stage.0, activation,
        ))
        .await
    }
    async fn submission_batch(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        let (queue, cursor, descriptor, indirect) = request.into_parts();
        let descriptor = pdma_model_device::DmaWrite::from(descriptor);
        let indirect = indirect.map(|(stage, write)| (stage.0, write.into()));
        self.submit_queue_batch(pdma_model_device::SubmissionBatch::new(
            queue, cursor, descriptor, indirect,
        ))
        .await
    }
    async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        self.read_mmio(offset, width).await
    }
    async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        self.write_mmio(offset, bytes).await
    }
    async fn wait_irq_notification_ready(&self) -> bool {
        self.wait_irq_notification_ready().await
    }
}
