use crate::register::{
    PdmaBatchError, PdmaClaimError, PdmaDmaWrite, PdmaMappedRegion, PdmaProvisioningCause,
    PdmaProvisioningError, PdmaRegisterAccessError,
};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::binding::PdmaMemoryRegion;
use pdma_model_device::{MmioBatchError, MmioRegisterError, PdmaMemoryError};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub(super) fn validate_alignment(alignment: MemoryAlignment) -> Result<(), PdmaProvisioningError> {
    let required =
        MemoryAlignment::try_new(64).map_err(|_| PdmaProvisioningError::InvalidProjection)?;
    if alignment == required {
        Ok(())
    } else {
        Err(PdmaProvisioningError::InvalidProjection)
    }
}

pub(super) fn project_region(
    region: PdmaMemoryRegion,
    alignment: MemoryAlignment,
) -> Result<PdmaMappedRegion, PdmaProvisioningError> {
    let base = MemoryAddress::new(region.base().raw());
    let storage = if region.is_empty() {
        None
    } else {
        Some(
            MemoryRegion::try_new(base, region.length(), alignment)
                .map_err(|_| PdmaProvisioningError::InvalidProjection)?,
        )
    };
    PdmaMappedRegion::try_new(region.object(), base, region.length(), alignment, storage)
}

pub(super) fn device_write(write: PdmaDmaWrite) -> pdma_model_device::DmaWrite {
    let (address, bytes) = write.into_parts();
    pdma_model_device::DmaWrite::new(
        pdma_model_contract::abi::DmaAddress::new(address.raw()),
        bytes,
    )
}

pub(super) const fn map_model_allocation(
    _: pdma_model_device::ModelError,
) -> PdmaProvisioningError {
    PdmaProvisioningError::Provisioning(PdmaProvisioningCause::AddressExhausted)
}

pub(super) const fn map_memory(error: PdmaMemoryError) -> PdmaProvisioningError {
    let cause = match error {
        PdmaMemoryError::AddressExhausted => PdmaProvisioningCause::AddressExhausted,
        PdmaMemoryError::StorageExhausted => PdmaProvisioningCause::StorageExhausted,
        PdmaMemoryError::OverlappingRegion => PdmaProvisioningCause::OverlappingRegion,
        PdmaMemoryError::InvalidRegion => PdmaProvisioningCause::InvalidRegion,
        PdmaMemoryError::StaleStage {
            expected_generation,
            actual_generation,
        } => PdmaProvisioningCause::StaleProposal {
            expected: expected_generation,
            actual: actual_generation,
        },
        PdmaMemoryError::Unmapped => PdmaProvisioningCause::Unmapped,
        PdmaMemoryError::CrossesRegion => PdmaProvisioningCause::CrossesRegion,
    };
    PdmaProvisioningError::Provisioning(cause)
}

pub(super) const fn map_register(error: MmioRegisterError) -> PdmaRegisterAccessError {
    match error {
        MmioRegisterError::Access(error) => PdmaRegisterAccessError::Access(error),
        MmioRegisterError::Observation(error) => PdmaRegisterAccessError::Observation(error),
    }
}

pub(super) const fn map_claim(error: MmioRegisterError) -> PdmaClaimError {
    match error {
        MmioRegisterError::Access(error) => PdmaClaimError::Access(error),
        MmioRegisterError::Observation(error) => PdmaClaimError::Observation(error),
    }
}

pub(super) const fn map_batch(error: MmioBatchError) -> PdmaBatchError {
    match error {
        MmioBatchError::InvalidShape => PdmaBatchError::InvalidShape,
        MmioBatchError::Access(error) => PdmaBatchError::Access(error),
        MmioBatchError::Dma(details) => {
            let cause = match details.result {
                MmioResult::DmaAddressExhausted => PdmaProvisioningCause::AddressExhausted,
                MmioResult::DmaStorageExhausted => PdmaProvisioningCause::StorageExhausted,
                MmioResult::DmaUnmapped => PdmaProvisioningCause::Unmapped,
                MmioResult::DmaCrossesRegion => PdmaProvisioningCause::CrossesRegion,
                MmioResult::DmaOverlap => PdmaProvisioningCause::OverlappingRegion,
                MmioResult::DmaStaleStage => PdmaProvisioningCause::StaleProposal {
                    expected: details.details[0],
                    actual: details.details[1],
                },
                _ => PdmaProvisioningCause::InvalidRegion,
            };
            PdmaBatchError::Provisioning(PdmaProvisioningError::Provisioning(cause))
        }
        MmioBatchError::Command(details) => PdmaBatchError::Result(details),
        MmioBatchError::Observation(error) => PdmaBatchError::Observation(error),
    }
}

pub(super) fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(super) fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
