use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount};
use pdma_model_contract::{DmaAddress, PdmaMemoryRegion, QueueActivation};
use pdma_model_device::{AsyncDeviceHandle, MmioBatchError, PdmaMemoryError, QueueResponseBytes};
use pdma_model_host::ModelResult;
use pdma_model_host::{AsyncDeviceBusStage, BusSubmission, MmioDeviceBus, MmioDeviceTransport};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueBatchKind {
    Bootstrap,
    Submission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmioAccessRecord {
    BatchQueueCommand {
        batch: QueueBatchKind,
        command: u32,
        offset: u32,
        bytes: [u8; 4],
    },
    Read {
        offset: u32,
        width: MmioByteCount,
    },
    Write {
        offset: u32,
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct RecordingMmioTransport {
    handle: AsyncDeviceHandle,
    accesses: Arc<Mutex<Vec<MmioAccessRecord>>>,
}

impl RecordingMmioTransport {
    pub fn new(handle: AsyncDeviceHandle) -> Self {
        Self {
            handle,
            accesses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn accesses(&self) -> Vec<MmioAccessRecord> {
        self.accesses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn record(&self, access: MmioAccessRecord) {
        self.accesses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(access);
    }

    fn record_batch_queue_command(&self, batch: QueueBatchKind, command: u32) {
        self.record(MmioAccessRecord::BatchQueueCommand {
            batch,
            command,
            offset: 0x168,
            bytes: command.to_le_bytes(),
        });
    }
}

pub type ScenarioBus = MmioDeviceBus<RecordingMmioTransport>;

impl MmioDeviceTransport for RecordingMmioTransport {
    type Stage = AsyncDeviceBusStage;

    async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        MmioDeviceTransport::allocate_memory_region(&self.handle, len).await
    }

    async fn stage_memory_regions(&self, lengths: &[u64]) -> Result<Self::Stage, PdmaMemoryError> {
        MmioDeviceTransport::stage_memory_regions(&self.handle, lengths).await
    }

    async fn read_dma(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, PdmaMemoryError> {
        MmioDeviceTransport::read_dma(&self.handle, address, length).await
    }

    async fn write_dma(&self, address: DmaAddress, bytes: &[u8]) -> Result<(), PdmaMemoryError> {
        MmioDeviceTransport::write_dma(&self.handle, address, bytes).await
    }

    async fn bootstrap_batch(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.record_batch_queue_command(QueueBatchKind::Bootstrap, 1);
        MmioDeviceTransport::bootstrap_batch(&self.handle, stage, activation).await
    }

    async fn submission_batch(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.record_batch_queue_command(QueueBatchKind::Submission, 3);
        MmioDeviceTransport::submission_batch(&self.handle, request).await
    }

    async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        self.record(MmioAccessRecord::Read { offset, width });
        MmioDeviceTransport::read_mmio(&self.handle, offset, width).await
    }

    async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        self.record(MmioAccessRecord::Write {
            offset,
            bytes: bytes.to_vec(),
        });
        MmioDeviceTransport::write_mmio(&self.handle, offset, bytes).await
    }

    async fn wait_irq_notification_ready(&self) -> bool {
        MmioDeviceTransport::wait_irq_notification_ready(&self.handle).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandOrigin {
    BootstrapBatch,
    SubmissionBatch,
    DirectQueue,
    DirectIrq,
}

fn command_sequence(accesses: &[MmioAccessRecord]) -> Vec<(CommandOrigin, u32)> {
    accesses
        .iter()
        .filter_map(|access| match access {
            MmioAccessRecord::BatchQueueCommand {
                batch,
                command,
                offset: 0x168,
                bytes,
            } if u32::from_le_bytes(*bytes) == *command => Some((
                match batch {
                    QueueBatchKind::Bootstrap => CommandOrigin::BootstrapBatch,
                    QueueBatchKind::Submission => CommandOrigin::SubmissionBatch,
                },
                *command,
            )),
            MmioAccessRecord::Write { offset, bytes } if *offset == 0x168 => bytes
                .as_slice()
                .try_into()
                .ok()
                .map(u32::from_le_bytes)
                .map(|command| (CommandOrigin::DirectQueue, command)),
            MmioAccessRecord::Write { offset, bytes } if *offset == 0x220 => bytes
                .as_slice()
                .try_into()
                .ok()
                .map(u32::from_le_bytes)
                .map(|command| (CommandOrigin::DirectIrq, command)),
            MmioAccessRecord::BatchQueueCommand { .. }
            | MmioAccessRecord::Read { .. }
            | MmioAccessRecord::Write { .. } => None,
        })
        .collect()
}

pub(super) fn assert_mmio_transport(accesses: &[MmioAccessRecord]) {
    assert!(matches!(
        accesses.first(),
        Some(MmioAccessRecord::BatchQueueCommand {
            batch: QueueBatchKind::Bootstrap,
            command: 1,
            offset: 0x168,
            bytes: [1, 0, 0, 0],
        })
    ));
    let batch_commands = accesses
        .iter()
        .filter_map(|access| match access {
            MmioAccessRecord::BatchQueueCommand {
                batch,
                command,
                offset,
                bytes,
            } => Some((*batch, *command, *offset, *bytes)),
            MmioAccessRecord::Read { .. } | MmioAccessRecord::Write { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        batch_commands,
        [
            (QueueBatchKind::Bootstrap, 1, 0x168, [1, 0, 0, 0]),
            (QueueBatchKind::Submission, 3, 0x168, [3, 0, 0, 0]),
        ]
    );
    let sequence = command_sequence(accesses);
    assert_eq!(
        sequence,
        [
            (CommandOrigin::BootstrapBatch, 1),
            (CommandOrigin::DirectIrq, 1),
            (CommandOrigin::DirectIrq, 2),
            (CommandOrigin::DirectIrq, 5),
            (CommandOrigin::DirectQueue, 2),
            (CommandOrigin::SubmissionBatch, 3),
            (CommandOrigin::DirectQueue, 2),
            (CommandOrigin::DirectQueue, 2),
            (CommandOrigin::DirectQueue, 4),
            (CommandOrigin::DirectIrq, 9),
            (CommandOrigin::DirectQueue, 2),
            (CommandOrigin::DirectQueue, 2),
            (CommandOrigin::DirectQueue, 7),
            (CommandOrigin::DirectQueue, 2),
            (CommandOrigin::DirectIrq, 6),
            (CommandOrigin::DirectQueue, 2),
        ]
    );
    let direct_commands = |origin| {
        sequence
            .iter()
            .filter_map(|(actual, command)| (*actual == origin).then_some(*command))
            .collect::<Vec<_>>()
    };
    let direct_queue_commands = direct_commands(CommandOrigin::DirectQueue);
    let direct_irq_commands = direct_commands(CommandOrigin::DirectIrq);
    assert!(matches!(
        accesses.last(),
        Some(MmioAccessRecord::Read {
            offset: 0x1c8,
            width: 8
        })
    ));
    println!(
        "manual-qa MMIO transport: accesses={} batch-carried=[bootstrap:ACTIVATE(1)@0x168 bytes=[1,0,0,0],submission:PUBLISH_SQ_PRODUCER(3)@0x168 bytes=[3,0,0,0]] direct-raw queue@0x168={direct_queue_commands:?} irq@0x220={direct_irq_commands:?} final-read=0x1c8/8",
        accesses.len(),
    );
}
