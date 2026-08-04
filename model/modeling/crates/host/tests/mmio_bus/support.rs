use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount};
use pdma_model_contract::abi::{
    DeviceEpoch, DmaAddress, DmaRegion, QueueActivation, QueueRingRegions, RingLayout, RingRecord,
};
use pdma_model_contract::{
    BufferId, EventRouting, IrqNotification, ModelResult, QueueConfig, QueueId, SoftwareOwnerId,
};
use pdma_model_device::{DmaSpaceError, MmioBatchError, QueueResponseBytes};
use pdma_model_host::{BusDmaStage, BusSubmission, MmioDeviceTransport};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Access {
    BootstrapBatch,
    SubmissionBatch,
    Read(u32, u128),
    Write(u32, Vec<u8>),
}

#[derive(Clone, Debug)]
struct RecordingStage(Vec<DmaRegion>);

impl BusDmaStage for RecordingStage {
    fn proposed_regions(&self) -> &[DmaRegion] {
        &self.0
    }
}

#[derive(Clone, Debug, Default)]
struct RecordingTransport {
    accesses: Arc<Mutex<Vec<Access>>>,
    gate_queue_start: Arc<AtomicBool>,
    queue_started: Arc<Notify>,
    queue_release: Arc<Notify>,
    fail_write: Arc<Mutex<Option<u32>>>,
    batch_failure: Arc<AtomicBool>,
    queue_result: Arc<AtomicU32>,
}

impl RecordingTransport {
    fn new() -> Self {
        Self::default()
    }

    fn accesses(&self) -> Vec<Access> {
        self.accesses
            .lock()
            .map_or_else(|_| Vec::new(), |value| value.clone())
    }

    fn accesses_json(&self) -> String {
        let entries = self
            .accesses()
            .into_iter()
            .map(|access| match access {
                Access::BootstrapBatch => {
                    String::from("{\"kind\":\"batch\",\"name\":\"bootstrap\"}")
                }
                Access::SubmissionBatch => {
                    String::from("{\"kind\":\"batch\",\"name\":\"submission\"}")
                }
                Access::Read(offset, width) => {
                    format!("{{\"kind\":\"read\",\"offset\":{offset},\"width\":{width}}}")
                }
                Access::Write(offset, bytes) => {
                    format!("{{\"kind\":\"write\",\"offset\":{offset},\"bytes\":{bytes:?}}}")
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    fn clear(&self) {
        if let Ok(mut accesses) = self.accesses.lock() {
            accesses.clear();
        }
    }

    fn gate_next_queue_start(&self) {
        self.gate_queue_start.store(true, Ordering::SeqCst);
    }

    async fn wait_queue_start(&self) {
        self.queue_started.notified().await;
    }

    fn fail_next_write(&self, offset: u32) {
        if let Ok(mut failure) = self.fail_write.lock() {
            *failure = Some(offset);
        }
    }

    fn fail_next_batch(&self) {
        self.batch_failure.store(true, Ordering::SeqCst);
    }

    fn set_queue_result(&self, result: u32) {
        self.queue_result.store(result, Ordering::SeqCst);
    }

    fn record(&self, access: Access) {
        if let Ok(mut accesses) = self.accesses.lock() {
            accesses.push(access);
        }
    }
}

impl MmioDeviceTransport for RecordingTransport {
    type Stage = RecordingStage;

    async fn allocate_dma_region(&self, _len: usize) -> ModelResult<DmaRegion> {
        Err(pdma_model_contract::ModelError::DmaAllocationExhausted)
    }

    async fn stage_dma_regions(&self, lengths: &[u64]) -> Result<Self::Stage, DmaSpaceError> {
        let bases = [0x1_0000, 0x2_0000, 0x3_0000];
        let regions = lengths
            .iter()
            .copied()
            .zip(bases)
            .enumerate()
            .map(|(index, (length, base))| {
                DmaRegion::try_new(BufferId::new(index), DmaAddress::new(base), length)
                    .map_err(|_| DmaSpaceError::InvalidRegion)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RecordingStage(regions))
    }

    async fn read_dma(&self, _address: DmaAddress, _length: u64) -> Result<Vec<u8>, DmaSpaceError> {
        Err(DmaSpaceError::Unmapped)
    }

    async fn write_dma(&self, _address: DmaAddress, _bytes: &[u8]) -> Result<(), DmaSpaceError> {
        Err(DmaSpaceError::Unmapped)
    }

    async fn bootstrap_batch(
        &self,
        _stage: Self::Stage,
        _activation: QueueActivation,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.record(Access::BootstrapBatch);
        if self.batch_failure.swap(false, Ordering::SeqCst) {
            return Err(MmioBatchError::InvalidShape);
        }
        Ok([0; 72])
    }

    async fn submission_batch(
        &self,
        _request: BusSubmission<Self::Stage>,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.record(Access::SubmissionBatch);
        Ok([0; 72])
    }

    async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        self.record(Access::Read(offset, width));
        let value = match offset {
            0x16c => u64::from(self.queue_result.load(Ordering::SeqCst).max(1)),
            0x224 => 1_u64,
            0x254 => 0x13,
            _ => 0,
        };
        match width {
            4 => Ok(u32::try_from(value)
                .map_or([0; 4], u32::to_le_bytes)
                .to_vec()),
            8 => Ok(value.to_le_bytes().to_vec()),
            _ => Err(MmioAccessError::WrongWidth {
                offset,
                expected: 0,
                actual: width,
            }),
        }
    }

    async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        self.record(Access::Write(offset, bytes.to_vec()));
        let should_fail = self
            .fail_write
            .lock()
            .is_ok_and(|mut failure| failure.take() == Some(offset));
        if should_fail {
            return Err(MmioAccessError::UnmappedOffset { offset });
        }
        if offset == 0x100 && self.gate_queue_start.swap(false, Ordering::SeqCst) {
            self.queue_started.notify_one();
            self.queue_release.notified().await;
        }
        Ok(())
    }

    async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        None
    }
}

fn activation(
    regions: &[DmaRegion],
) -> Result<QueueActivation, Box<dyn std::error::Error + Send + Sync>> {
    let [submission, completion, event] = regions else {
        return Err("three regions required".into());
    };
    let rings = QueueRingRegions::new(
        RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
        RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
        RingLayout::try_new(*event, 2, RingRecord::Event)?,
    );
    Ok(QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        rings,
    )?)
}

fn expected_queue_transcript() -> Vec<Access> {
    let mut accesses = Vec::new();
    append_status(&mut accesses);
    for command in 4_u32..=9 {
        append_status(&mut accesses);
        accesses.push(Access::Write(0x110, 0_u64.to_le_bytes().to_vec()));
        accesses.push(Access::Write(0x118, 1_u64.to_le_bytes().to_vec()));
        append_command(&mut accesses, command);
    }
    accesses
}

fn append_status(accesses: &mut Vec<Access>) {
    accesses.push(Access::Write(0x100, 0_u64.to_le_bytes().to_vec()));
    accesses.push(Access::Write(0x108, 0_u64.to_le_bytes().to_vec()));
    append_command(accesses, 2);
}

fn append_command(accesses: &mut Vec<Access>, command: u32) {
    accesses.push(Access::Write(0x168, command.to_le_bytes().to_vec()));
    accesses.push(Access::Read(0x16c, 4));
    for offset in (0x188..=0x1c8).step_by(8) {
        accesses.push(Access::Read(offset, 8));
    }
}

#[path = "cases.rs"]
mod cases;
