use super::RegisterBanks;
use crate::runner::bus::BusControlError;
use crate::runner::reset::reset_runner;
use crate::runner::state::RunnerState;
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
use pdma_model_contract::{AbiControlError, DeviceEpoch, WholeDeviceReset};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runner) enum ResetOutcome {
    Success(DeviceEpoch),
    Rejected(BusControlError, MmioResultDetails),
}

impl ResetOutcome {
    pub(in crate::runner) const fn typed_result(self) -> Result<DeviceEpoch, BusControlError> {
        match self {
            Self::Success(epoch) => Ok(epoch),
            Self::Rejected(error, _) => Err(error),
        }
    }
}

pub(in crate::runner) fn execute(state: &mut RunnerState, expected_epoch: u64) -> ResetOutcome {
    match reset_runner(
        state,
        WholeDeviceReset::new(DeviceEpoch::new(expected_epoch)),
    ) {
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

const fn map_error(error: BusControlError) -> MmioResultDetails {
    match error {
        BusControlError::StaleEpoch { expected, actual } => {
            details(MmioResult::StaleEpoch, [expected.raw(), actual.raw(), 0])
        }
        BusControlError::Abi(AbiControlError::DeviceEpochOverflow { current }) => {
            details(MmioResult::DeviceEpochOverflow, [current, 0, 0])
        }
        BusControlError::Abi(
            AbiControlError::UnsupportedVersion { .. }
            | AbiControlError::UnknownFeatureBits { .. }
            | AbiControlError::InvalidRingDepth { .. }
            | AbiControlError::RingExtentOverflow { .. }
            | AbiControlError::MisalignedRing { .. }
            | AbiControlError::RingExtentMismatch { .. }
            | AbiControlError::CursorRegression { .. }
            | AbiControlError::CursorUpdateRegression { .. }
            | AbiControlError::CursorDistanceExceeded { .. }
            | AbiControlError::CursorIncrementOverflow { .. }
            | AbiControlError::UnexpectedRingRecord { .. }
            | AbiControlError::QueueDepthMismatch { .. }
            | AbiControlError::EventDepthMismatch { .. }
            | AbiControlError::UnknownStatusValue { .. },
        )
        | BusControlError::Dma(_)
        | BusControlError::InvalidQueue
        | BusControlError::InvalidTransaction => details(MmioResult::InvalidTransaction, [4, 0, 0]),
    }
}

pub(in crate::runner) fn register_banks(state: &mut RunnerState) {
    state.mmio = RegisterBanks::new();
}
