use super::TestResult;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    DmaAddress, RingLayout,
};
use pdma_model_contract::semantic::{
    CompletionRingId, EpochIrqSource, EventRouting, IrqSource, QueueActivation, QueueConfig,
    QueueControl, QueueCursorUpdate, QueueId, QueueRingObjects, RingCursor, SoftwareOwnerId,
};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker};

pub(in crate::queue) struct Fixture {
    pub(in crate::queue) handle: AsyncDeviceHandle,
    worker: Option<AsyncDeviceWorker>,
    pub(in crate::queue) queue: EpochQueue,
    pub(in crate::queue) submission: DmaAddress,
    pub(in crate::queue) completion: DmaAddress,
    pub(in crate::queue) event: DmaAddress,
}

impl Fixture {
    pub(in crate::queue) async fn new(epoch: DeviceEpoch, result_depth: u64) -> TestResult<Self> {
        let (runner, worker) = AsyncDevice::new(
            pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
                hardware_interface_runtime::SharedMemory::new(),
            )),
            pdma_model_contract::semantic::ResourceConfig::default(),
        );
        let handle = runner.handle();
        let command_depth = 4;
        let stage = handle
            .stage_memory_regions(&[
                RingLayout::<AbiCommand>::checked_extent(command_depth)?,
                RingLayout::<AbiCompletion>::checked_extent(result_depth)?,
                RingLayout::<AbiEvent>::checked_extent(result_depth)?,
            ])
            .await?;
        let [submission, completion, event] = stage.regions() else {
            return Err("ring stage shape".into());
        };
        let (submission, completion, event) = (*submission, *completion, *event);
        let config = QueueConfig::new(
            usize::try_from(command_depth)?,
            usize::try_from(result_depth)?,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        );
        let activation = QueueActivation::new(
            epoch,
            QueueId::new(0),
            config,
            QueueRingObjects::new(submission.object(), completion.object(), event.object()),
        );
        handle.commit_memory_stage(stage).await?;
        handle.activate_mapped_queue(activation).await?;
        Ok(Self {
            handle,
            worker: Some(worker),
            queue: activation.queue(),
            submission: submission.base(),
            completion: completion.base(),
            event: event.base(),
        })
    }

    pub(in crate::queue) const fn completion_source(&self) -> EpochIrqSource {
        EpochIrqSource::new(
            self.queue.epoch(),
            IrqSource::Completion(CompletionRingId::new(self.queue.queue_id().raw())),
        )
    }

    pub(in crate::queue) fn start(
        &mut self,
    ) -> TestResult<tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>> {
        let worker = self.worker.take().ok_or("worker already started")?;
        Ok(tokio::spawn(worker.run()))
    }

    pub(in crate::queue) async fn submit_copy(
        &self,
        producer: u64,
        command: u64,
        source: DmaAddress,
        destination: DmaAddress,
    ) -> TestResult {
        let descriptor = AbiCommand::from_payload(
            command,
            AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                source_dma_address: source.raw(),
                destination_dma_address: destination.raw(),
                length: 4.into(),
            }),
        );
        let mut bytes = [0_u8; 64];
        descriptor.encode(&mut bytes)?;
        let address = self.submission.checked_add(producer * 64)?;
        self.handle.write_memory(address, &bytes).await?;
        self.handle
            .control_queue(QueueControl::PublishSubmissionProducer(
                QueueCursorUpdate::new(self.queue, RingCursor::new(producer + 1)),
            ))
            .await?;
        Ok(())
    }

    pub(super) async fn submit_bytes(&self, producer: u64, bytes: [u8; 64]) -> TestResult {
        let address = self.submission.checked_add(producer * 64)?;
        self.handle.write_memory(address, &bytes).await?;
        self.handle
            .control_queue(QueueControl::PublishSubmissionProducer(
                QueueCursorUpdate::new(self.queue, RingCursor::new(producer + 1)),
            ))
            .await?;
        Ok(())
    }

    pub(in crate::queue) async fn capture(&self, producer: u64) -> TestResult {
        self.handle
            .control_queue(QueueControl::CaptureSubmissionProducer(
                QueueCursorUpdate::new(self.queue, RingCursor::new(producer)),
            ))
            .await?;
        Ok(())
    }

    pub(in crate::queue) async fn status(&self) -> TestResult<pdma_model_device::QueueStatus> {
        Ok(self
            .handle
            .control_queue(QueueControl::ReadStatus(self.queue))
            .await?)
    }

    pub(in crate::queue) async fn consume_completion(&self, consumer: u64) -> TestResult {
        self.handle
            .control_queue(QueueControl::PublishCompletionConsumer(
                QueueCursorUpdate::new(self.queue, RingCursor::new(consumer)),
            ))
            .await?;
        Ok(())
    }
}

pub(in crate::queue) async fn allocate_pair(
    handle: &AsyncDeviceHandle,
) -> TestResult<(
    pdma_model_device::MappedDmaRegion,
    pdma_model_device::MappedDmaRegion,
)> {
    let stage = handle.stage_memory_regions(&[4, 4]).await?;
    let [source, destination] = stage.regions() else {
        return Err("operand stage shape".into());
    };
    let pair = (*source, *destination);
    handle.commit_memory_stage(stage).await?;
    handle.write_memory(pair.0.base(), &[1, 3, 5, 7]).await?;
    Ok(pair)
}
