use super::{
    AsyncDeviceBusStage, AsyncPdmaBus, BusDmaStage, BusQueueStatus, BusSubmission,
    QueueControlWrite,
};
use crate::ModelResult;
use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_contract::{
    DmaAddress, EpochQueue, PdmaMemoryRegion, QueueActivation, QueueControl,
};
use pdma_model_contract::{EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot};
use pdma_model_device::{
    AsyncDeviceHandle, BusControlError, PdmaMemoryError, QueueStatus, SubmissionAcceptance,
};

impl BusDmaStage for AsyncDeviceBusStage {
    fn proposed_regions(&self) -> &[PdmaMemoryRegion] {
        self.0.regions()
    }
}

impl From<QueueStatus> for BusQueueStatus {
    fn from(status: QueueStatus) -> Self {
        Self::new(
            status.queue(),
            status.submission(),
            status.captured_submission_producer(),
            status.completion(),
            status.event(),
        )
    }
}

impl AsyncPdmaBus for AsyncDeviceHandle {
    type Stage = AsyncDeviceBusStage;
    type DmaError = PdmaMemoryError;
    type ControlError = BusControlError;

    async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        Self::allocate_memory_region(self, len)
            .await
            .map_err(Into::into)
    }

    async fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> Result<AsyncDeviceBusStage, PdmaMemoryError> {
        Self::stage_memory_regions(self, lengths)
            .await
            .map(AsyncDeviceBusStage)
    }

    async fn read_dma(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, PdmaMemoryError> {
        Self::read_memory(self, address, length).await
    }

    async fn write_dma(&self, address: DmaAddress, bytes: &[u8]) -> Result<(), PdmaMemoryError> {
        Self::write_memory(self, address, bytes).await
    }

    async fn bootstrap_queue(
        &self,
        stage: AsyncDeviceBusStage,
        activation: QueueActivation,
    ) -> Result<BusQueueStatus, BusControlError> {
        Self::activate_queue(self, stage.0, activation)
            .await
            .map(Into::into)
    }

    async fn accept_submission(
        &self,
        request: BusSubmission<AsyncDeviceBusStage>,
    ) -> Result<BusQueueStatus, BusControlError> {
        let (queue, producer, descriptor, indirect) = request.into_parts();
        let translated = SubmissionAcceptance::new(
            queue,
            producer,
            descriptor.into(),
            indirect.map(|(stage, write)| (stage.0, write.into())),
        );
        Self::accept_submission(self, translated)
            .await
            .map(Into::into)
    }

    async fn read_queue_status(
        &self,
        queue: EpochQueue,
    ) -> Result<BusQueueStatus, BusControlError> {
        Self::control_queue(self, QueueControl::ReadStatus(queue))
            .await
            .map(Into::into)
    }

    async fn write_queue_control(
        &self,
        request: QueueControlWrite,
    ) -> Result<BusQueueStatus, BusControlError> {
        Self::control_queue(self, request.into_control())
            .await
            .map(Into::into)
    }

    async fn configure_irq_route(
        &self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        Self::configure_irq_route(self, source, route)
            .await
            .map_err(Into::into)
    }

    async fn enable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        Self::enable_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn disable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        Self::disable_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn mask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        Self::mask_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn unmask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        Self::unmask_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn ack_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        Self::ack_irq_source(self, source).await.map_err(Into::into)
    }

    async fn observe_irq_source(&self, source: EpochIrqSource) -> ModelResult<IrqSourceStatus> {
        Self::observe_irq_source(self, source)
            .await
            .map_err(Into::into)
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        Self::irq_snapshot(self).await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        Self::wait_irq(self).await
    }

    async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        Self::wait_irq_notification(self).await
    }
}
