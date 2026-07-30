#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeStage(Vec<DmaRegion>);

impl BusDmaStage for FakeStage {
    fn proposed_regions(&self) -> &[DmaRegion] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Sequence {
    Allocate(usize, DmaRegion),
    Stage(Vec<u64>, Vec<DmaRegion>),
    Bootstrap(Vec<DmaRegion>, QueueActivation),
    Write(DmaAddress, Vec<u8>),
    Read(DmaAddress, u64),
    WriteControl(QueueControlWrite),
    ReadControl(EpochQueue),
    Accept(
        EpochQueue,
        RingCursor,
        Vec<DmaRegion>,
        BusDmaWrite,
        Option<BusDmaWrite>,
    ),
    WaitIrq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FakeError;

impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("recording bus failure")
    }
}

impl std::error::Error for FakeError {}

#[derive(Debug, Default)]
struct FakeState {
    sequence: Vec<Sequence>,
    stage_count: usize,
}

#[derive(Debug, Clone, Default)]
struct FakeBus {
    state: Arc<Mutex<FakeState>>,
}

impl FakeBus {
    fn new() -> Self {
        Self::default()
    }

    fn sequence(&self) -> Vec<Sequence> {
        self.state
            .lock()
            .map_or_else(|_| Vec::new(), |state| state.sequence.clone())
    }

    fn record(&self, entry: Sequence) -> Result<(), FakeError> {
        self.state
            .lock()
            .map_err(|_| FakeError)?
            .sequence
            .push(entry);
        Ok(())
    }
}

impl AsyncPdmaBus for FakeBus {
    type Stage = FakeStage;
    type DmaError = FakeError;
    type ControlError = FakeError;

    async fn allocate_dma_region(&self, len: usize) -> ModelResult<DmaRegion> {
        let length = u64::try_from(len).map_err(|_| ModelError::DmaAllocationExhausted)?;
        let region = DmaRegion::try_new(BufferId::new(0), DmaAddress::new(0x1_0000), length)
            .map_err(|_| ModelError::DmaAllocationExhausted)?;
        self.record(Sequence::Allocate(len, region))
            .map_err(|_| ModelError::DmaAllocationExhausted)?;
        Ok(region)
    }

    async fn stage_dma_regions(&self, lengths: &[u64]) -> Result<Self::Stage, Self::DmaError> {
        let mut state = self.state.lock().map_err(|_| FakeError)?;
        let ordinal = state.stage_count;
        state.stage_count += 1;
        let base = if ordinal == 0 { 0x2_0000 } else { 0x3_0000 };
        let mut cursor = base;
        let regions = lengths
            .iter()
            .copied()
            .enumerate()
            .map(|(index, length)| {
                let region = DmaRegion::try_new(
                    BufferId::new(index + 1 + ordinal * 3),
                    DmaAddress::new(cursor),
                    length,
                )
                .map_err(|_| FakeError)?;
                cursor += length.max(64);
                Ok(region)
            })
            .collect::<Result<Vec<_>, FakeError>>()?;
        state
            .sequence
            .push(Sequence::Stage(lengths.to_vec(), regions.clone()));
        drop(state);
        Ok(FakeStage(regions))
    }

    async fn read_dma(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, Self::DmaError> {
        self.record(Sequence::Read(address, length))?;
        Ok(vec![1, 3, 5, 7])
    }

    async fn write_dma(&self, address: DmaAddress, bytes: &[u8]) -> Result<(), Self::DmaError> {
        self.record(Sequence::Write(address, bytes.to_vec()))
    }

    async fn bootstrap_queue(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> Result<BusQueueStatus, Self::ControlError> {
        self.record(Sequence::Bootstrap(stage.0, activation))?;
        zero_status(activation.queue(), activation.rings())
    }

    async fn accept_submission(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> Result<BusQueueStatus, Self::ControlError> {
        let queue = request.queue();
        let producer = request.expected_producer();
        let descriptor = request.descriptor().clone();
        let (regions, indirect) = request
            .indirect()
            .map_or((Vec::new(), None), |(stage, write)| {
                (stage.proposed_regions().to_vec(), Some(write.clone()))
            });
        self.record(Sequence::Accept(
            queue, producer, regions, descriptor, indirect,
        ))?;
        empty_status(queue)
    }

    async fn read_queue_status(
        &self,
        queue: EpochQueue,
    ) -> Result<BusQueueStatus, Self::ControlError> {
        self.record(Sequence::ReadControl(queue))?;
        empty_status(queue)
    }

    async fn write_queue_control(
        &self,
        request: QueueControlWrite,
    ) -> Result<BusQueueStatus, Self::ControlError> {
        self.record(Sequence::WriteControl(request))?;
        empty_status(request.queue())
    }

    async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        self.record(Sequence::WaitIrq).ok()?;
        Some(IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(0),
            IrqVector::new(9),
            pdma_model_contract::EpochIrqSource::new(
                DeviceEpoch::initial(),
                pdma_model_contract::IrqSource::Event(QueueId::new(0)),
            ),
        )))
    }
}

fn activation(
    regions: &[DmaRegion],
    queue_id: QueueId,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission_region, completion_region, event_region] = regions else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::try_from(regions.len()).unwrap_or(u32::MAX),
            },
        );
    };
    let submission = RingLayout::try_new(*submission_region, 2, RingRecord::Submission)?;
    let completion = RingLayout::try_new(*completion_region, 2, RingRecord::Completion)?;
    let event = RingLayout::try_new(*event_region, 2, RingRecord::Event)?;
    QueueActivation::try_new(
        DeviceEpoch::initial(),
        queue_id,
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingRegions::new(submission, completion, event),
    )
}

fn zero_status(queue: EpochQueue, rings: QueueRingRegions) -> Result<BusQueueStatus, FakeError> {
    let zero = RingCursor::new(0);
    Ok(BusQueueStatus::new(
        queue,
        RingCursors::try_new(rings.submission(), zero, zero).map_err(|_| FakeError)?,
        zero,
        RingCursors::try_new(rings.completion(), zero, zero).map_err(|_| FakeError)?,
        RingCursors::try_new(rings.event(), zero, zero).map_err(|_| FakeError)?,
    ))
}

fn empty_status(queue: EpochQueue) -> Result<BusQueueStatus, FakeError> {
    let region = DmaRegion::try_new(BufferId::new(99), DmaAddress::new(0x8_0000), 64)
        .map_err(|_| FakeError)?;
    let layout = RingLayout::try_new(region, 1, RingRecord::Submission).map_err(|_| FakeError)?;
    let cursors = RingCursors::try_new(layout, RingCursor::new(0), RingCursor::new(0))
        .map_err(|_| FakeError)?;
    Ok(BusQueueStatus::new(
        queue,
        cursors,
        RingCursor::new(0),
        cursors,
        cursors,
    ))
}

fn expected_sequence(
    allocated: DmaRegion,
    proposed: &[DmaRegion],
    activation: QueueActivation,
) -> Result<Vec<Sequence>, pdma_model_contract::abi::AbiAddressError> {
    let [submission, _, _] = proposed else {
        return Ok(Vec::new());
    };
    let indirect = DmaRegion::try_new(BufferId::new(4), DmaAddress::new(0x3_0000), 8)?;
    let queue = activation.queue();
    Ok(vec![
        Sequence::Allocate(32, allocated),
        Sequence::Stage(vec![128, 64, 128], proposed.to_vec()),
        Sequence::Bootstrap(proposed.to_vec(), activation),
        Sequence::Write(allocated.base(), vec![1, 3, 5, 7]),
        Sequence::Read(allocated.base(), 4),
        Sequence::WriteControl(QueueControlWrite::CaptureSubmissionProducer(
            QueueCursorUpdate::new(queue, RingCursor::new(0)),
        )),
        Sequence::ReadControl(queue),
        Sequence::Stage(vec![8], vec![indirect]),
        Sequence::Accept(
            queue,
            RingCursor::new(0),
            vec![indirect],
            BusDmaWrite::new(submission.base(), vec![0x5a; 64]),
            Some(BusDmaWrite::new(indirect.base(), vec![0xa5; 8])),
        ),
        Sequence::WaitIrq,
    ])
}
