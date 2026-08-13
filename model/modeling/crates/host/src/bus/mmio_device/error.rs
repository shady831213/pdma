use crate::BusControlErrorAdapter;
use crate::ModelError;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResult, MmioResultDetails};
use pdma_model_contract::{DeviceEpoch, EpochQueue};
use pdma_model_device::MmioBatchError;

/// Exact failure returned by an MMIO queue procedure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioBusControlError {
    /// A byte-bearing access failed structurally.
    Access(MmioAccessError),
    /// A closed ordered batch failed before commit.
    Batch(MmioBatchError),
    /// A command completed with a semantic error result.
    Result(MmioResultDetails),
    /// A successful response bank was not exactly decodable.
    MalformedResponse,
}

impl core::fmt::Display for MmioBusControlError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "MMIO bus control failed: {self:?}")
    }
}

impl std::error::Error for MmioBusControlError {}

impl From<MmioAccessError> for MmioBusControlError {
    fn from(error: MmioAccessError) -> Self {
        Self::Access(error)
    }
}

impl From<MmioBatchError> for MmioBusControlError {
    fn from(error: MmioBatchError) -> Self {
        Self::Batch(error)
    }
}

impl BusControlErrorAdapter for MmioBusControlError {
    fn into_model_error(self, queue: EpochQueue) -> ModelError {
        match self {
            Self::Result(MmioResultDetails {
                result: MmioResult::StaleEpoch,
                details: [expected, actual, _],
            }) => ModelError::StaleDeviceEpoch {
                expected: DeviceEpoch::new(expected),
                actual: DeviceEpoch::new(actual),
            },
            Self::Result(MmioResultDetails {
                result: MmioResult::InvalidQueue,
                ..
            }) => ModelError::UnknownQueue(queue.queue_id()),
            Self::Batch(MmioBatchError::Dma(details)) => dma_result(details.result),
            Self::Access(_)
            | Self::Batch(
                MmioBatchError::InvalidShape
                | MmioBatchError::Access(_)
                | MmioBatchError::Command(_)
                | MmioBatchError::Observation(_),
            )
            | Self::Result(_)
            | Self::MalformedResponse => ModelError::DmaSubmissionState,
        }
    }
}

const fn dma_result(result: MmioResult) -> ModelError {
    match result {
        MmioResult::DmaAddressExhausted | MmioResult::DmaStorageExhausted => {
            ModelError::DmaAllocationExhausted
        }
        MmioResult::NotExecuted
        | MmioResult::Ok
        | MmioResult::Empty
        | MmioResult::UnsupportedVersion
        | MmioResult::UnknownFeatureBits
        | MmioResult::StaleEpoch
        | MmioResult::InvalidQueue
        | MmioResult::InvalidIrqSource
        | MmioResult::IrqSourceActive
        | MmioResult::InvalidRingDepth
        | MmioResult::RingExtentOverflow
        | MmioResult::MisalignedRing
        | MmioResult::RingExtentMismatch
        | MmioResult::UnexpectedRingRecord
        | MmioResult::QueueDepthMismatch
        | MmioResult::EventDepthMismatch
        | MmioResult::CursorRegression
        | MmioResult::CursorUpdateRegression
        | MmioResult::CursorDistanceExceeded
        | MmioResult::CursorIncrementOverflow
        | MmioResult::DeviceEpochOverflow
        | MmioResult::UnknownStatusValue
        | MmioResult::DmaUnmapped
        | MmioResult::DmaCrossesRegion
        | MmioResult::DmaInvalidRegion
        | MmioResult::DmaOverlap
        | MmioResult::DmaStaleStage
        | MmioResult::InvalidTransaction
        | MmioResult::UnknownCommand
        | MmioResult::UnknownEncoding => ModelError::DmaSubmissionState,
    }
}
