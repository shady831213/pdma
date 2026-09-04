use super::{QueueRequest, status_response};
use crate::device::DeviceCore;
use crate::dma::PdmaMemoryError;
use crate::queue::exact::{PdmaControlError, QueueStatus};
use pdma_model_contract::abi::AbiControlError;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueCommand, QueueResponseBank, ResponseUpdate,
};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochQueue, QueueControl, QueueCursorUpdate, QueueId, RingCursor, SemanticError,
};

pub(super) type CommandResult = Result<(QueueResponseBank, bool), MmioResultDetails>;

pub(super) fn execute(
    state: &mut DeviceCore,
    staging: QueueRequest,
    raw_command: u32,
) -> (MmioResultDetails, ResponseUpdate<QueueResponseBank>, bool) {
    let Ok(command) = QueueCommand::try_from(raw_command) else {
        return (
            fail(MmioResult::UnknownCommand, [u64::from(raw_command), 0, 0]),
            ResponseUpdate::Preserve,
            false,
        );
    };
    match run(state, staging, command) {
        Ok((response, changed)) => (
            fail(MmioResult::Ok, [0; 3]),
            ResponseUpdate::Replace(response),
            changed,
        ),
        Err(error) => (error, ResponseUpdate::Preserve, false),
    }
}

fn run(state: &mut DeviceCore, staging: QueueRequest, command: QueueCommand) -> CommandResult {
    if command == QueueCommand::Activate {
        return super::queue_config::activate(state, staging);
    }
    let queue_id = usize::try_from(staging.id)
        .map(QueueId::new)
        .map_err(|_| invalid_transaction(command))?;
    let queue = EpochQueue::new(DeviceEpoch::new(staging.expected_epoch), queue_id);
    if command == QueueCommand::ReadStatus {
        let (result, _) = state.control_exact_queue(QueueControl::ReadStatus(queue));
        return result
            .map_err(|error| map_bus(error, staging.id, command.code(), 0))
            .and_then(|status| status_response(status).map(|response| (response, false)));
    }
    let current = current_status(state, queue, staging.id, command)?;
    let role = cursor_role(command);
    if selected_cursor(current, command).raw() != staging.expected_cursor {
        return Err(invalid_transaction(command));
    }
    let update = QueueCursorUpdate::new(queue, RingCursor::new(staging.new_cursor));
    let request = cursor_request(command, update)?;
    let (result, changed) = state.control_exact_queue(request);
    let status = result.map_err(|error| map_bus(error, staging.id, command.code(), role))?;
    Ok((status_response(status)?, changed))
}

fn current_status(
    state: &mut DeviceCore,
    queue: EpochQueue,
    queue_id: u64,
    command: QueueCommand,
) -> Result<QueueStatus, MmioResultDetails> {
    let (result, _) = state.control_exact_queue(QueueControl::ReadStatus(queue));
    result.map_err(|error| map_bus(error, queue_id, command.code(), cursor_role(command)))
}

fn cursor_request(
    command: QueueCommand,
    update: QueueCursorUpdate,
) -> Result<QueueControl, MmioResultDetails> {
    match command {
        QueueCommand::PublishSqProducer => Ok(QueueControl::PublishSubmissionProducer(update)),
        QueueCommand::CaptureSqProducer => Ok(QueueControl::CaptureSubmissionProducer(update)),
        QueueCommand::PublishSqConsumer => Ok(QueueControl::PublishSubmissionConsumer(update)),
        QueueCommand::PublishCqProducer => Ok(QueueControl::PublishCompletionProducer(update)),
        QueueCommand::PublishCqConsumer => Ok(QueueControl::PublishCompletionConsumer(update)),
        QueueCommand::PublishEqProducer => Ok(QueueControl::PublishEventProducer(update)),
        QueueCommand::PublishEqConsumer => Ok(QueueControl::PublishEventConsumer(update)),
        QueueCommand::Activate | QueueCommand::ReadStatus => Err(invalid_transaction(command)),
    }
}

const fn selected_cursor(status: QueueStatus, command: QueueCommand) -> RingCursor {
    match command {
        QueueCommand::PublishSqProducer => status.submission().producer(),
        QueueCommand::CaptureSqProducer => status.captured_submission_producer(),
        QueueCommand::PublishSqConsumer => status.submission().consumer(),
        QueueCommand::PublishCqProducer => status.completion().producer(),
        QueueCommand::PublishCqConsumer => status.completion().consumer(),
        QueueCommand::PublishEqProducer => status.event().producer(),
        QueueCommand::PublishEqConsumer => status.event().consumer(),
        QueueCommand::Activate | QueueCommand::ReadStatus => RingCursor::new(0),
    }
}

const fn cursor_role(command: QueueCommand) -> u64 {
    match command {
        QueueCommand::PublishSqProducer | QueueCommand::Activate | QueueCommand::ReadStatus => 0,
        QueueCommand::CaptureSqProducer => 1,
        QueueCommand::PublishSqConsumer => 2,
        QueueCommand::PublishCqProducer => 3,
        QueueCommand::PublishCqConsumer => 4,
        QueueCommand::PublishEqProducer => 5,
        QueueCommand::PublishEqConsumer => 6,
    }
}

pub(super) const fn fail(result: MmioResult, details: [u64; 3]) -> MmioResultDetails {
    MmioResultDetails { result, details }
}

fn invalid_transaction(command: QueueCommand) -> MmioResultDetails {
    fail(
        MmioResult::InvalidTransaction,
        [(1_u64 << 32) | u64::from(command.code()), 0, 0],
    )
}

pub(crate) fn map_bus(
    error: PdmaControlError,
    queue_id: u64,
    command: u32,
    role: u64,
) -> MmioResultDetails {
    match error {
        PdmaControlError::Dma(error) => map_dma(error),
        PdmaControlError::Abi(error) => map_abi(error, role),
        PdmaControlError::Semantic(error) => map_semantic(error, role),
        PdmaControlError::StaleEpoch { expected, actual } => {
            fail(MmioResult::StaleEpoch, [expected.raw(), actual.raw(), 0])
        }
        PdmaControlError::InvalidQueue => fail(MmioResult::InvalidQueue, [queue_id, 0, 0]),
        PdmaControlError::InvalidTransaction => fail(
            MmioResult::InvalidTransaction,
            [(1_u64 << 32) | u64::from(command), 0, 0],
        ),
    }
}

fn map_semantic(error: SemanticError, role: u64) -> MmioResultDetails {
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

const fn map_dma(error: PdmaMemoryError) -> MmioResultDetails {
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
