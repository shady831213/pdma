use super::TestResult;
use pdma_model_contract::{
    AbiCommand, AbiCopy, AbiId, CopyIovas, DeviceEpoch, DmaAddress, EpochQueue, QueueActivation,
    QueueControl, QueueCursorUpdate, QueueRingRegions, RingCursor, RingLayout, RingRecord,
};
use pdma_model_contract::{
    CompletionRingId, EpochIrqSource, EventRouting, IrqSource, QueueConfig, QueueId,
    SoftwareOwnerId,
};
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, DmaWrite, SubmissionAcceptance,
};

pub(super) struct Fixture {
    pub(super) handle: AsyncDeviceHandle,
    worker: Option<AsyncDeviceWorker>,
    pub(super) queue: EpochQueue,
    pub(super) submission: DmaAddress,
    pub(super) completion: DmaAddress,
    pub(super) event: DmaAddress,
}

impl Fixture {
    pub(super) async fn new(epoch: DeviceEpoch, result_depth: u64) -> TestResult<Self> {
        let (runner, worker) = AsyncDevice::queue_port();
        let handle = runner.handle();
        let command_depth = 4;
        let stage = handle
            .stage_dma_regions(&[
                command_depth * u64::from(RingRecord::Submission.record_size()),
                result_depth * u64::from(RingRecord::Completion.record_size()),
                result_depth * u64::from(RingRecord::Event.record_size()),
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
        let activation = QueueActivation::try_new(
            epoch,
            QueueId::new(0),
            config,
            QueueRingRegions::new(
                RingLayout::try_new(submission, command_depth, RingRecord::Submission)?,
                RingLayout::try_new(completion, result_depth, RingRecord::Completion)?,
                RingLayout::try_new(event, result_depth, RingRecord::Event)?,
            ),
        )?;
        handle.activate_queue(stage, activation).await?;
        Ok(Self {
            handle,
            worker: Some(worker),
            queue: activation.queue(),
            submission: submission.base(),
            completion: completion.base(),
            event: event.base(),
        })
    }

    pub(super) const fn completion_source(&self) -> EpochIrqSource {
        EpochIrqSource::new(
            self.queue.epoch(),
            IrqSource::Completion(CompletionRingId::new(self.queue.queue_id().raw())),
        )
    }

    pub(super) fn start(
        &mut self,
    ) -> TestResult<tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>> {
        let worker = self.worker.take().ok_or("worker already started")?;
        Ok(tokio::spawn(worker.run()))
    }

    pub(super) async fn submit_copy(
        &self,
        producer: u64,
        command: u64,
        source: DmaAddress,
        destination: DmaAddress,
    ) -> TestResult {
        let descriptor = AbiCommand::Copy(AbiCopy::new(
            AbiId::new(command),
            CopyIovas::new(source, destination),
            4,
        ));
        let mut bytes = [0_u8; 64];
        descriptor.encode(&mut bytes)?;
        let address = self.submission.checked_add(producer * 64)?;
        self.handle
            .accept_submission(SubmissionAcceptance::new(
                self.queue,
                RingCursor::new(producer),
                DmaWrite::new(address, bytes.to_vec()),
                None,
            ))
            .await?;
        Ok(())
    }

    pub(super) async fn submit_raw(&self, producer: u64, bytes: [u8; 64]) -> TestResult {
        let address = self.submission.checked_add(producer * 64)?;
        self.handle
            .accept_submission(SubmissionAcceptance::new(
                self.queue,
                RingCursor::new(producer),
                DmaWrite::new(address, bytes.to_vec()),
                None,
            ))
            .await?;
        Ok(())
    }

    pub(super) async fn capture(&self, producer: u64) -> TestResult {
        self.handle
            .control_queue(QueueControl::CaptureSubmissionProducer(
                QueueCursorUpdate::new(self.queue, RingCursor::new(producer)),
            ))
            .await?;
        Ok(())
    }

    pub(super) async fn status(&self) -> TestResult<pdma_model_device::QueueStatus> {
        Ok(self
            .handle
            .control_queue(QueueControl::ReadStatus(self.queue))
            .await?)
    }

    pub(super) async fn consume_completion(&self, consumer: u64) -> TestResult {
        self.handle
            .control_queue(QueueControl::PublishCompletionConsumer(
                QueueCursorUpdate::new(self.queue, RingCursor::new(consumer)),
            ))
            .await?;
        Ok(())
    }
}

pub(super) async fn allocate_pair(
    handle: &AsyncDeviceHandle,
) -> TestResult<(
    pdma_model_contract::bridge::DmaRegion,
    pdma_model_contract::bridge::DmaRegion,
)> {
    let stage = handle.stage_dma_regions(&[4, 4]).await?;
    let [source, destination] = stage.regions() else {
        return Err("operand stage shape".into());
    };
    let pair = (*source, *destination);
    handle.commit_dma_stage(stage).await?;
    handle.write_iova(pair.0.base(), &[1, 3, 5, 7]).await?;
    Ok(pair)
}
