use pdma_model_contract::abi::mmio::{MmioByteCount, QueueResponseBank};
use pdma_model_contract::abi::{RingCursor, RingRecord};
use pdma_model_contract::bridge::{
    PdmaMemoryRegion, QueueActivation, QueueControl, QueueRingRegions, RingLayout,
};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue};
use pdma_model_contract::semantic::{EventRouting, QueueConfig, QueueId, SoftwareOwnerId};
use pdma_model_device::{
    AsyncDeviceHandle, DmaWrite, PdmaMemoryStage, QueueBootstrapBatch, SubmissionBatch,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct RecordingObserver(Mutex<Vec<hardware_interface_runtime::RegisterObservation>>);

impl RecordingObserver {
    pub fn snapshot(&self) -> Vec<hardware_interface_runtime::RegisterObservation> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl hardware_interface_runtime::RegisterObserver for RecordingObserver {
    fn observe(&self, observation: hardware_interface_runtime::RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

#[derive(Debug)]
pub struct GatedObserver {
    observations: Mutex<Vec<hardware_interface_runtime::RegisterObservation>>,
    expected_regions: AtomicUsize,
    memory: Arc<hardware_interface_runtime::SharedMemory>,
    entered: std::sync::Barrier,
    release: std::sync::Barrier,
}

impl GatedObserver {
    pub const fn new(memory: Arc<hardware_interface_runtime::SharedMemory>) -> Self {
        Self {
            observations: Mutex::new(Vec::new()),
            expected_regions: AtomicUsize::new(3),
            memory,
            entered: std::sync::Barrier::new(2),
            release: std::sync::Barrier::new(2),
        }
    }

    pub fn expect_regions(&self, count: usize) {
        self.expected_regions.store(count, Ordering::SeqCst);
    }

    pub fn await_observation(&self) {
        self.entered.wait();
    }

    pub fn release_observation(&self) {
        self.release.wait();
    }

    pub fn snapshot(&self) -> Vec<hardware_interface_runtime::RegisterObservation> {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl hardware_interface_runtime::RegisterObserver for GatedObserver {
    fn observe(&self, observation: hardware_interface_runtime::RegisterObservation) {
        assert_eq!(
            self.memory.regions().len(),
            self.expected_regions.load(Ordering::SeqCst)
        );
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
        self.entered.wait();
        self.release.wait();
    }
}

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub fn new_handle() -> AsyncDeviceHandle {
    let (device, _worker) =
        pdma_model_device::AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
            std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
        ));
    device.handle()
}

pub fn observed_handle() -> (AsyncDeviceHandle, Arc<RecordingObserver>) {
    let (handle, observer, _) = observed_handle_with_memory();
    (handle, observer)
}

pub fn observed_handle_with_memory() -> (
    AsyncDeviceHandle,
    Arc<RecordingObserver>,
    Arc<hardware_interface_runtime::SharedMemory>,
) {
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (handle, observer) = observed_handle_for_memory(&memory);
    (handle, observer, memory)
}

pub fn observed_handle_for_memory(
    memory: &Arc<hardware_interface_runtime::SharedMemory>,
) -> (AsyncDeviceHandle, Arc<RecordingObserver>) {
    let observer = Arc::new(RecordingObserver::default());
    let (device, _worker) =
        pdma_model_device::AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            Arc::clone(memory),
            pdma_model_contract::semantic::ResourceConfig::default(),
            observer.clone(),
        ));
    (device.handle(), observer)
}

pub fn gated_handle() -> (
    AsyncDeviceHandle,
    Arc<GatedObserver>,
    Arc<hardware_interface_runtime::SharedMemory>,
) {
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(GatedObserver::new(Arc::clone(&memory)));
    let (device, _worker) =
        pdma_model_device::AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            Arc::clone(&memory),
            pdma_model_contract::semantic::ResourceConfig::default(),
            observer.clone(),
        ));
    (device.handle(), observer, memory)
}

pub fn activation(
    stage: &PdmaMemoryStage,
    epoch: DeviceEpoch,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission, completion, event] = stage.regions() else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::try_from(stage.regions().len()).unwrap_or(u32::MAX),
            },
        );
    };
    QueueActivation::try_new(
        epoch,
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingRegions::new(
            RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
            RingLayout::try_new(*event, 2, RingRecord::Event)?,
        ),
    )
}

pub async fn bootstrap(
    handle: &AsyncDeviceHandle,
) -> TestResult<(EpochQueue, PdmaMemoryRegion, [u8; 72])> {
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    let submission = *submission;
    let activation = activation(&stage, DeviceEpoch::initial())?;
    let response = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
        .await?;
    Ok((activation.queue(), submission, response))
}

pub fn response(status: pdma_model_device::QueueStatus) -> QueueResponseBank {
    QueueResponseBank {
        epoch: status.queue().epoch().raw().into(),
        queue_id: u64::try_from(status.queue().queue_id().raw())
            .unwrap_or(u64::MAX)
            .into(),
        sq_producer: status.submission().producer().raw().into(),
        sq_captured: status.captured_submission_producer().raw().into(),
        sq_consumer: status.submission().consumer().raw().into(),
        cq_producer: status.completion().producer().raw().into(),
        cq_consumer: status.completion().consumer().raw().into(),
        eq_producer: status.event().producer().raw().into(),
        eq_consumer: status.event().consumer().raw().into(),
    }
}

pub async fn queue_latches(handle: &AsyncDeviceHandle) -> TestResult<Vec<Vec<u8>>> {
    let mut values = Vec::new();
    for (offset, width) in [
        (0x16c, MmioByteCount::from(4_u8)),
        (0x170, MmioByteCount::from(8_u8)),
        (0x178, MmioByteCount::from(8_u8)),
        (0x180, MmioByteCount::from(8_u8)),
        (0x188, MmioByteCount::from(8_u8)),
    ] {
        values.push(handle.read_mmio(offset, width).await?);
    }
    Ok(values)
}

pub async fn queue_status(
    handle: &AsyncDeviceHandle,
    queue: EpochQueue,
) -> TestResult<pdma_model_device::QueueStatus> {
    Ok(handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?)
}

pub fn submission(
    queue: EpochQueue,
    expected: RingCursor,
    address: pdma_model_contract::abi::DmaAddress,
    byte: u8,
) -> SubmissionBatch {
    SubmissionBatch::new(
        queue,
        expected,
        DmaWrite::new(address, vec![byte; 64]),
        None,
    )
}
