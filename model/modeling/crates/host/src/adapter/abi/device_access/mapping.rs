use crate::adapter::abi::{
    PdmaMappedRegion, PdmaProvisioningCause, PdmaProvisioningError, PdmaRegisterAccessError,
};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_device::MappedDmaRegion;
use pdma_model_device::{DeviceError, MmioRegisterError};
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
    region: MappedDmaRegion,
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

pub(super) const fn map_model_allocation(_: DeviceError) -> PdmaProvisioningError {
    PdmaProvisioningError::Provisioning(PdmaProvisioningCause::AddressExhausted)
}

pub(super) const fn map_register(error: MmioRegisterError) -> PdmaRegisterAccessError {
    match error {
        MmioRegisterError::Access(error) => PdmaRegisterAccessError::Access(error),
        MmioRegisterError::Observation(error) => PdmaRegisterAccessError::Observation(error),
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
