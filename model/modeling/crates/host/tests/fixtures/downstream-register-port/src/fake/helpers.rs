use super::FakeSnapshot;
use hardware_interface_runtime::{
    AccessOrigin, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion,
    RegisterObservation,
};
use pdma_model_contract::abi::mmio::{decode_read, decode_write};
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqMessage, IrqMessageId, IrqNotification,
    IrqSource, IrqVector,
};
use pdma_model_host::adapter::abi::{
    PdmaProvisioningCause, PdmaProvisioningError, PdmaRegisterAccessError,
};
use std::sync::{Mutex, MutexGuard};

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
