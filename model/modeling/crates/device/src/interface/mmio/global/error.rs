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
