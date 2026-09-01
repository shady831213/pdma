//! Exact MMIO reset command execution.

use crate::device::DeviceCore;
use crate::queue::exact::PdmaControlError;
use pdma_model_contract::abi::AbiControlError;
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResetOutcome {
    Success(DeviceEpoch),
    Rejected(PdmaControlError, MmioResultDetails),
}

impl ResetOutcome {
    pub(crate) const fn typed_result(self) -> Result<DeviceEpoch, PdmaControlError> {
        match self {
            Self::Success(epoch) => Ok(epoch),
            Self::Rejected(error, _) => Err(error),
        }
    }
}

pub(crate) fn execute(state: &mut DeviceCore, expected_epoch: u64) -> ResetOutcome {
    match state.reset_runner(WholeDeviceReset::new(DeviceEpoch::new(expected_epoch))) {
        Ok(epoch) => ResetOutcome::Success(epoch),
        Err(error) => ResetOutcome::Rejected(error, map_error(error)),
    }
}

pub(super) const fn details(result: MmioResult, values: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails {
        result,
        details: values,
    }
}

const fn map_error(error: PdmaControlError) -> MmioResultDetails {
    match error {
        PdmaControlError::StaleEpoch { expected, actual } => {
            details(MmioResult::StaleEpoch, [expected.raw(), actual.raw(), 0])
        }
        PdmaControlError::Semantic(
            pdma_model_contract::semantic::SemanticError::DeviceEpochOverflow { current },
        ) => details(MmioResult::DeviceEpochOverflow, [current, 0, 0]),
        PdmaControlError::Abi(
            AbiControlError::UnsupportedVersion { .. }
            | AbiControlError::UnknownFeatureBits { .. }
            | AbiControlError::InvalidRingDepth { .. }
            | AbiControlError::RingExtentOverflow { .. }
            | AbiControlError::MisalignedRing { .. }
            | AbiControlError::RingExtentMismatch { .. }
            | AbiControlError::CursorRegression { .. }
            | AbiControlError::CursorDistanceExceeded { .. }
            | AbiControlError::CursorUpdateRegression { .. }
            | AbiControlError::CursorIncrementOverflow { .. }
            | AbiControlError::QueueDepthMismatch { .. }
            | AbiControlError::EventDepthMismatch { .. }
            | AbiControlError::UnknownStatusValue { .. },
        )
        | PdmaControlError::Dma(_)
        | PdmaControlError::Semantic(_)
        | PdmaControlError::InvalidQueue
        | PdmaControlError::InvalidTransaction => {
            details(MmioResult::InvalidTransaction, [4, 0, 0])
        }
    }
}
