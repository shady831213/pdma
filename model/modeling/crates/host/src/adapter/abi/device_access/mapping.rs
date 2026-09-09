use crate::PortError;
use crate::adapter::abi::{PdmaInterruptSourceError, PdmaRegisterAccessError};
use crate::semantic::{PortBackendError, PortBackendKind};
use pdma_model_contract::semantic::SemanticError;
use pdma_model_device::PdmaControlError;
use pdma_model_device::{DeviceError, MmioRegisterError};

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
        PdmaControlError::Memory(error) => {
            PortError::Backend(PortBackendError::new(PortBackendKind::Memory, error))
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
        | DeviceError::UnknownQueue(_)
        | DeviceError::UnknownCompletionRing(_)
        | DeviceError::UnknownIrqSource(_)
        | DeviceError::IrqSourceActive(_)
        | DeviceError::MemoryFault(_)
        | DeviceError::RangeOverflow
        | DeviceError::DmaSubmissionState => PdmaInterruptSourceError::InvalidState,
    }
}
