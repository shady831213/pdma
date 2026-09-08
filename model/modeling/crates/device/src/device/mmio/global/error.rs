use crate::queue::exact::PdmaControlError;
use pdma_model_contract::abi::AbiControlError;
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};

pub(super) fn map_discovery(error: AbiControlError) -> MmioResultDetails {
    match error {
        AbiControlError::UnsupportedVersion { requested } => {
            details(MmioResult::UnsupportedVersion, [u64::from(requested), 0, 0])
        }
        AbiControlError::UnknownFeatureBits { bits } => {
            details(MmioResult::UnknownFeatureBits, [bits, 0, 0])
        }
        AbiControlError::InvalidRingDepth { .. }
        | AbiControlError::RingExtentOverflow { .. }
        | AbiControlError::MisalignedRing { .. }
        | AbiControlError::RingExtentMismatch { .. }
        | AbiControlError::CursorRegression { .. }
        | AbiControlError::CursorDistanceExceeded { .. }
        | AbiControlError::CursorUpdateRegression { .. }
        | AbiControlError::CursorIncrementOverflow { .. }
        | AbiControlError::QueueDepthMismatch { .. }
        | AbiControlError::EventDepthMismatch { .. }
        | AbiControlError::UnknownStatusValue { .. } => {
            details(MmioResult::InvalidTransaction, [1, 0, 0])
        }
    }
}

const fn details(result: MmioResult, values: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails {
        result,
        details: values,
    }
}

pub(super) const fn map_reset(error: PdmaControlError) -> MmioResultDetails {
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
