use super::status_response;
use crate::queue::QueueBlock;
use crate::queue::exact::{PdmaControlError, QueueStatus};
use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails, QueueCommand, ResponseUpdate};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochQueue, QueueControl, QueueCursorUpdate, QueueId, RingCursor,
};

pub(super) use super::errors::map_abi;
use super::errors::{map_dma, map_semantic};

type CommandResult = Result<(QueueStatus, bool), MmioResultDetails>;

pub(super) fn execute(queues: &mut QueueBlock, epoch: DeviceEpoch) -> (bool, Option<QueueId>) {
    let raw_command = queues.registers.command.raw();
    let Ok(command) = QueueCommand::try_from(raw_command) else {
        queues.complete(
            fail(MmioResult::UnknownCommand, [u64::from(raw_command), 0, 0]),
            ResponseUpdate::Preserve,
        );
        return (false, None);
    };
    let result = run(queues, epoch, command).and_then(|(status, changed)| {
        status_response(status).map(|response| (response, status.queue().queue_id(), changed))
    });
    match result {
        Ok((response, queue_id, changed)) => {
            let runnable = match command {
                QueueCommand::CaptureSqProducer => Some(queue_id),
                QueueCommand::Activate
                | QueueCommand::ReadStatus
                | QueueCommand::PublishSqProducer
                | QueueCommand::PublishSqConsumer
                | QueueCommand::PublishCqProducer
                | QueueCommand::PublishCqConsumer
                | QueueCommand::PublishEqProducer
                | QueueCommand::PublishEqConsumer => None,
            };
            queues.complete(
                fail(MmioResult::Ok, [0; 3]),
                ResponseUpdate::Replace(response),
            );
            (changed, runnable)
        }
        Err(error) => {
            queues.complete(error, ResponseUpdate::Preserve);
            (false, None)
        }
    }
}

fn run(queues: &mut QueueBlock, epoch: DeviceEpoch, command: QueueCommand) -> CommandResult {
    let staging = queues.registers;
    let queue_id = usize::try_from(staging.id.raw())
        .map(QueueId::new)
        .map_err(|_| invalid_transaction(command))?;
    let queue = EpochQueue::new(DeviceEpoch::new(staging.expected_epoch.raw()), queue_id);
    let current = current_status(queues, epoch, queue, command)?;
    match command {
        QueueCommand::ReadStatus => return Ok((current, false)),
        QueueCommand::Activate => return Err(invalid_transaction(command)),
        QueueCommand::PublishSqProducer
        | QueueCommand::CaptureSqProducer
        | QueueCommand::PublishSqConsumer
        | QueueCommand::PublishCqProducer
        | QueueCommand::PublishCqConsumer
        | QueueCommand::PublishEqProducer
        | QueueCommand::PublishEqConsumer => {}
    }
    let role = cursor_role(command);
    if selected_cursor(current, command).raw() != staging.expected_cursor.raw() {
        return Err(invalid_transaction(command));
    }
    let update = QueueCursorUpdate::new(queue, RingCursor::new(staging.new_cursor.raw()));
    let request = cursor_request(command, update)?;
    let (result, changed) = queues.control_exact_queue(epoch, request);
    let status = result.map_err(|error| map_bus(error, staging.id.raw(), command.code(), role))?;
    Ok((status, changed))
}

fn current_status(
    queues: &mut QueueBlock,
    epoch: DeviceEpoch,
    queue: EpochQueue,
    command: QueueCommand,
) -> Result<QueueStatus, MmioResultDetails> {
    let (result, _) = queues.control_exact_queue(epoch, QueueControl::ReadStatus(queue));
    result.map_err(|error| {
        map_bus(
            error,
            queues.registers.id.raw(),
            command.code(),
            cursor_role(command),
        )
    })
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
