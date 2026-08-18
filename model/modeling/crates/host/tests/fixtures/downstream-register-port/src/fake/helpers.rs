use super::{FakeSnapshot, TestResult};
use hardware_interface_runtime::{
    AccessOrigin, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion,
    RegisterObservation,
};
use pdma_model_contract::abi::mmio::{QueueResponseBank, decode_read, decode_write};
use pdma_model_contract::semantic::{CompletionRingId, DeviceEpoch, EpochIrqSource, IrqMessage, IrqMessageId, IrqNotification, IrqSource, IrqVector, QueueConfig, QueueId, SoftwareOwnerId};
use pdma_model_contract::abi::{RingRecord};
use pdma_model_contract::abi::{DmaAddress};
use pdma_model_contract::bridge::{QueueActivation, QueueRingRegions, RingLayout};
use pdma_model_host::register::{
    PdmaBatchError, PdmaMappedRegion, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegisterAccessError,
};
use std::sync::{Mutex, MutexGuard};

pub fn activation(regions: &[PdmaMappedRegion]) -> TestResult<QueueActivation> {
    let [submission, completion, event] = regions else {
        return Err("three ring regions required".into());
    };
    let rings = QueueRingRegions::new(
        RingLayout::try_new(project(*submission)?, 2, RingRecord::Submission)?,
        RingLayout::try_new(project(*completion)?, 2, RingRecord::Completion)?,
        RingLayout::try_new(project(*event)?, 2, RingRecord::Event)?,
    );
    Ok(QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(
            2,
            2,
            pdma_model_contract::semantic::EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ),
        rings,
    )?)
}

pub fn message() -> IrqNotification {
    IrqNotification::Message(IrqMessage::new(
        IrqMessageId::new(0),
        IrqVector::new(41),
        EpochIrqSource::new(
            DeviceEpoch::initial(),
            IrqSource::Completion(CompletionRingId::new(0)),
        ),
    ))
}

pub(super) fn claim_observations() -> Vec<RegisterObservation> {
    let reads: [(u32, &[u8]); 10] = [
        (0x224, &[1, 0, 0, 0]),
        (0x240, &[0, 0, 0, 0, 0, 0, 0, 0]),
        (0x248, &[0, 0, 0, 0, 0, 0, 0, 0]),
        (0x250, &[0, 0, 0, 0]),
        (0x254, &[0x4b, 0, 0, 0]),
        (0x258, &[1, 0, 0, 0]),
        (0x25c, &[2, 0, 0, 0]),
        (0x260, &[0x29, 0, 0, 0, 0, 0, 0, 0]),
        (0x268, &[0, 0, 0, 0, 0, 0, 0, 0]),
        (0x270, &[0, 0, 0, 0, 0, 0, 0, 0]),
    ];
    let mut observations = vec![RegisterObservation::write(
        0,
        AccessOrigin::Direct,
        0x220,
        &[9, 0, 0, 0],
    )];
    observations.extend(
        reads
            .into_iter()
            .enumerate()
            .map(|(index, (offset, bytes))| {
                RegisterObservation::read(index as u64 + 1, AccessOrigin::Direct, offset, bytes)
            }),
    );
    observations
}

pub(super) fn response(
    epoch: u64,
    queue: usize,
    producer: u64,
) -> Result<[u8; 72], PdmaBatchError> {
    QueueResponseBank {
        epoch,
        queue_id: u64::try_from(queue).map_err(|_| PdmaBatchError::InvalidShape)?,
        sq_producer: producer,
        ..QueueResponseBank::default()
    }
    .to_le_bytes()
    .map_err(|_| PdmaBatchError::InvalidShape)
}

fn project(region: PdmaMappedRegion) -> TestResult<pdma_model_contract::bridge::PdmaMemoryRegion> {
    Ok(pdma_model_contract::bridge::PdmaMemoryRegion::try_new(
        region.object(),
        DmaAddress::new(region.base().raw()),
        region.length(),
    )?)
}

pub(super) fn validate_read(offset: u32, width: usize) -> Result<(), PdmaRegisterAccessError> {
    decode_read(offset, width as u128)
        .map(drop)
        .map_err(PdmaRegisterAccessError::Access)
}

pub(super) fn validate_write(offset: u32, bytes: &[u8]) -> Result<(), PdmaRegisterAccessError> {
    decode_write(offset, bytes)
        .map(drop)
        .map_err(PdmaRegisterAccessError::Access)
}

pub(super) fn align(value: u64, alignment: u64) -> Result<u64, PdmaProvisioningError> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or(PdmaProvisioningError::Provisioning(
            PdmaProvisioningCause::AddressExhausted,
        ))
}

pub(super) fn crossing(
    address: MemoryAddress,
    length: usize,
    state: &FakeSnapshot,
) -> MemoryAccessError {
    let region = state
        .mappings
        .iter()
        .find_map(|mapping| mapping.storage())
        .unwrap_or_else(|| {
            MemoryRegion::try_new(MemoryAddress::new(64), 1, MemoryAlignment::ONE).unwrap()
        });
    MemoryAccessError::CrossesRegion {
        address,
        length: u64::try_from(length).unwrap_or(u64::MAX),
        region,
    }
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
