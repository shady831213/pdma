//! Shared reset compatibility fixtures.

use std::sync::{Arc, Mutex};

use hardware_interface_runtime::{RegisterObservation, RegisterObserver, SharedMemory};
use pdma_model_contract::abi::RingRecord;
use pdma_model_contract::binding::{
    PdmaMemoryRegion, QueueActivation, QueueRingRegions, RingLayout,
};
use pdma_model_contract::semantic::{
    CommandId, CopyCommand, CopyRequest, DeviceEpoch, EventRouting, ExternalCommand, QueueConfig,
    QueueConfigHandle, ResourceConfig, SoftwareOwnerId,
};
use pdma_model_device::compat::SyncDevice;
use pdma_model_device::{AsyncDeviceHandle, DeviceCore};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) fn require_error<T, E>(result: Result<T, E>, message: &'static str) -> TestResult<E> {
    match result {
        Ok(_) => Err(message.into()),
        Err(error) => Ok(error),
    }
}

#[derive(Debug, Default)]
pub(super) struct RecordingObserver(Mutex<Vec<RegisterObservation>>);

impl RecordingObserver {
    pub(super) fn snapshot(&self) -> Vec<RegisterObservation> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl RegisterObserver for RecordingObserver {
    fn observe(&self, observation: RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

pub(super) fn core(
    memory: Arc<SharedMemory>,
    resources: ResourceConfig,
    observer: Arc<dyn RegisterObserver>,
) -> DeviceCore {
    DeviceCore::new(memory, resources, observer)
}

pub(super) const fn queue_config() -> QueueConfig {
    QueueConfig::new(2, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}

pub(super) fn activation(
    regions: &[PdmaMemoryRegion],
    epoch: DeviceEpoch,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission, completion, event] = regions else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::MAX,
            },
        );
    };
    QueueActivation::try_new(
        epoch,
        pdma_model_contract::semantic::QueueId::new(0),
        queue_config(),
        QueueRingRegions::new(
            RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(*completion, 1, RingRecord::Completion)?,
            RingLayout::try_new(*event, 1, RingRecord::Event)?,
        ),
    )
}

pub(super) fn submit_copy(
    device: &mut SyncDevice,
    queue: QueueConfigHandle,
    source: pdma_model_contract::semantic::BufferId,
    destination: pdma_model_contract::semantic::BufferId,
    command: usize,
) -> TestResult {
    device.submit_command(
        queue,
        ExternalCommand::Copy(CopyCommand::new(
            CommandId::new(command),
            queue.queue_id(),
            queue.completion_ring_id(),
            CopyRequest::new(source, destination, 1),
        )),
    )?;
    device.ring_doorbell(queue)?;
    Ok(())
}

pub(super) async fn read_u64(handle: &AsyncDeviceHandle, offset: u32) -> TestResult<u64> {
    let bytes = handle.read_mmio(offset, 8).await?;
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| "MMIO read did not return eight bytes")?;
    Ok(u64::from_le_bytes(bytes))
}

pub(super) async fn write_u32(handle: &AsyncDeviceHandle, offset: u32, value: u32) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub(super) async fn write_u64(handle: &AsyncDeviceHandle, offset: u32, value: u64) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub(super) async fn raw_reset(handle: &AsyncDeviceHandle, expected_epoch: u64) -> TestResult {
    write_u64(handle, 0x018, expected_epoch).await?;
    write_u32(handle, 0x020, 4).await
}
