use super::FakeSnapshot;
use hardware_interface_runtime::{MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion};
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
