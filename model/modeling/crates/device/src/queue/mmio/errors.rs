use super::queue_control::fail;
use crate::memory::PdmaMemoryError;
use pdma_model_contract::abi::AbiControlError;
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
use pdma_model_contract::semantic::SemanticError;

pub(super) fn map_semantic(error: SemanticError, role: u64) -> MmioResultDetails {
    match error {
        SemanticError::CursorRegression { producer, consumer } => {
            fail(MmioResult::CursorRegression, [producer, consumer, role])
        }
        SemanticError::CursorUpdateRegression { current, proposed } => fail(
            MmioResult::CursorUpdateRegression,
            [current, proposed, role],
        ),
        SemanticError::CursorDistanceExceeded {
            producer,
            consumer,
            depth,
        } => fail(
            MmioResult::CursorDistanceExceeded,
            [producer, consumer, u64::from(depth)],
        ),
        SemanticError::CursorIncrementOverflow { cursor, increment } => fail(
            MmioResult::CursorIncrementOverflow,
            [cursor, increment, role],
        ),
        SemanticError::DeviceEpochOverflow { current } => {
            fail(MmioResult::DeviceEpochOverflow, [current, 0, 0])
        }
        SemanticError::ZeroCapacity | SemanticError::RingFull => {
            fail(MmioResult::InvalidTransaction, [role, 0, 0])
        }
    }
}

pub(super) const fn map_dma(error: PdmaMemoryError) -> MmioResultDetails {
    match error {
        PdmaMemoryError::AddressExhausted => fail(MmioResult::DmaAddressExhausted, [0; 3]),
        PdmaMemoryError::StorageExhausted => fail(MmioResult::DmaStorageExhausted, [0; 3]),
        PdmaMemoryError::OverlappingRegion => fail(MmioResult::DmaOverlap, [0; 3]),
        PdmaMemoryError::InvalidRegion => fail(MmioResult::DmaInvalidRegion, [0; 3]),
        PdmaMemoryError::StaleStage {
            expected_generation,
            actual_generation,
        } => fail(
            MmioResult::DmaStaleStage,
            [expected_generation, actual_generation, 0],
        ),
        PdmaMemoryError::Unmapped => fail(MmioResult::DmaUnmapped, [0; 3]),
        PdmaMemoryError::CrossesRegion => fail(MmioResult::DmaCrossesRegion, [0; 3]),
    }
}

pub(super) fn map_abi(error: AbiControlError, role: u64) -> MmioResultDetails {
    match error {
        AbiControlError::InvalidRingDepth { depth, maximum } => fail(
            MmioResult::InvalidRingDepth,
            [depth, u64::from(maximum), role],
        ),
        AbiControlError::RingExtentOverflow { depth, record_size } => fail(
            MmioResult::RingExtentOverflow,
            [depth, u64::from(record_size), role],
        ),
        AbiControlError::MisalignedRing { base, alignment } => {
            fail(MmioResult::MisalignedRing, [base, alignment, role])
        }
        AbiControlError::RingExtentMismatch { expected, actual } => {
            fail(MmioResult::RingExtentMismatch, [expected, actual, role])
        }
        AbiControlError::CursorRegression { producer, consumer } => {
            fail(MmioResult::CursorRegression, [producer, consumer, role])
        }
        AbiControlError::CursorDistanceExceeded {
            producer,
            consumer,
            depth,
        } => fail(
            MmioResult::CursorDistanceExceeded,
            [producer, consumer, u64::from(depth)],
        ),
        AbiControlError::CursorUpdateRegression { current, proposed } => fail(
            MmioResult::CursorUpdateRegression,
            [current, proposed, role],
        ),
        AbiControlError::CursorIncrementOverflow { cursor, increment } => fail(
            MmioResult::CursorIncrementOverflow,
            [cursor, increment, role],
        ),
        AbiControlError::QueueDepthMismatch {
            expected, actual, ..
        } => fail(
            MmioResult::QueueDepthMismatch,
            [
                u64::try_from(expected).unwrap_or(u64::MAX),
                u64::from(actual),
                role,
            ],
        ),
        AbiControlError::EventDepthMismatch { expected, actual } => fail(
            MmioResult::EventDepthMismatch,
            [
                u64::try_from(expected).unwrap_or(u64::MAX),
                u64::from(actual),
                2,
            ],
        ),
        AbiControlError::UnsupportedVersion { requested } => {
            fail(MmioResult::UnsupportedVersion, [u64::from(requested), 0, 0])
        }
        AbiControlError::UnknownFeatureBits { bits } => {
            fail(MmioResult::UnknownFeatureBits, [bits, 0, 0])
        }
        AbiControlError::UnknownStatusValue { value } => {
            fail(MmioResult::UnknownStatusValue, [u64::from(value), 0, 0])
        }
    }
}
