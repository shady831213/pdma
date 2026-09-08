use crate::PortError;
use crate::adapter::abi::{
    PdmaInterruptSourceError, PdmaMappedRegion, PdmaProvisioningCause, PdmaProvisioningError,
    PdmaRegisterAccessError,
};
use crate::semantic::{PortBackendError, PortBackendKind};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::semantic::SemanticError;
use pdma_model_device::MappedDmaRegion;
use pdma_model_device::{DeviceError, MmioRegisterError};
use pdma_model_device::{PdmaControlError, PdmaMemoryError};
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

pub(super) fn map_register(error: MmioRegisterError) -> PdmaRegisterAccessError {
    match error {
        MmioRegisterError::Access(error) => PdmaRegisterAccessError::Access(error),
        MmioRegisterError::Observation(error) => PdmaRegisterAccessError::Observation(error),
        MmioRegisterError::CommittedObservation(error) => {
            PdmaRegisterAccessError::CommittedObservation(error)
        }
        MmioRegisterError::StaleEpoch { expected, actual } => {
            PdmaRegisterAccessError::StaleEpoch { expected, actual }
        }
        MmioRegisterError::Control(error) => PdmaRegisterAccessError::Control(map_control(error)),
    }
}

fn map_control(error: PdmaControlError) -> PortError {
    match error {
        PdmaControlError::StaleEpoch { expected, actual } => {
            PortError::StaleEpoch { expected, actual }
        }
        PdmaControlError::InvalidQueue | PdmaControlError::InvalidTransaction => {
            PortError::SubmissionState
        }
        PdmaControlError::Abi(error) => {
            PortError::Backend(PortBackendError::new(PortBackendKind::Register, error))
        }
        PdmaControlError::Semantic(SemanticError::ZeroCapacity) => PortError::ZeroCapacity,
        PdmaControlError::Semantic(SemanticError::RingFull) => PortError::RingFull,
        PdmaControlError::Semantic(
            error @ (SemanticError::CursorRegression { .. }
            | SemanticError::CursorUpdateRegression { .. }
            | SemanticError::CursorDistanceExceeded { .. }
            | SemanticError::CursorIncrementOverflow { .. }
            | SemanticError::DeviceEpochOverflow { .. }),
        ) => PortError::Backend(PortBackendError::new(PortBackendKind::Register, error)),
        PdmaControlError::Dma(error) => {
            let cause = match error {
                PdmaMemoryError::AddressExhausted => PdmaProvisioningCause::AddressExhausted,
                PdmaMemoryError::StorageExhausted => PdmaProvisioningCause::StorageExhausted,
                PdmaMemoryError::OverlappingRegion => PdmaProvisioningCause::OverlappingRegion,
                PdmaMemoryError::InvalidRegion | PdmaMemoryError::StaleStage { .. } => {
                    PdmaProvisioningCause::InvalidRegion
                }
                PdmaMemoryError::Unmapped => PdmaProvisioningCause::Unmapped,
                PdmaMemoryError::CrossesRegion => PdmaProvisioningCause::CrossesRegion,
            };
            PortError::Backend(PortBackendError::new(PortBackendKind::Provisioning, cause))
        }
    }
}

pub(super) const fn map_interrupt(error: DeviceError) -> PdmaInterruptSourceError {
    match error {
        DeviceError::StaleDeviceEpoch { expected, actual } => {
            PdmaInterruptSourceError::StaleEpoch { expected, actual }
        }
        DeviceError::ZeroCapacity
        | DeviceError::RingFull
        | DeviceError::UnknownBuffer(_)
        | DeviceError::UnknownQueue(_)
        | DeviceError::UnknownCompletionRing(_)
        | DeviceError::UnknownIrqSource(_)
        | DeviceError::IrqSourceActive(_)
        | DeviceError::RangeOverflow
        | DeviceError::OutOfBounds { .. }
        | DeviceError::DmaAllocationExhausted
        | DeviceError::DmaSubmissionState => PdmaInterruptSourceError::InvalidState,
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
