use crate::abi::mmio::QueueCommand;
use crate::abi::mmio::register::queue::QueueResponseRegistersValues;
pub use crate::abi::mmio::register::queue::{
    QueueResponseRegisters, QueueResponseRegistersRegisterId, QueueResponseRegistersView,
    QueueResponseRegistersViewMut,
};

/// Exact typed queue response register bank.
pub type QueueResponseBank = QueueResponseRegistersValues;

/// Closed successful queue command responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueResponse {
    /// `ACTIVATE` publishes all named queue response fields.
    Activate(QueueResponseBank),
    /// `READ_STATUS` publishes all named queue response fields.
    ReadStatus(QueueResponseBank),
    /// `PUBLISH_SQ_PRODUCER` publishes all named queue response fields.
    PublishSqProducer(QueueResponseBank),
    /// `CAPTURE_SQ_PRODUCER` publishes all named queue response fields.
    CaptureSqProducer(QueueResponseBank),
    /// `PUBLISH_SQ_CONSUMER` publishes all named queue response fields.
    PublishSqConsumer(QueueResponseBank),
    /// `PUBLISH_CQ_PRODUCER` publishes all named queue response fields.
    PublishCqProducer(QueueResponseBank),
    /// `PUBLISH_CQ_CONSUMER` publishes all named queue response fields.
    PublishCqConsumer(QueueResponseBank),
    /// `PUBLISH_EQ_PRODUCER` publishes all named queue response fields.
    PublishEqProducer(QueueResponseBank),
    /// `PUBLISH_EQ_CONSUMER` publishes all named queue response fields.
    PublishEqConsumer(QueueResponseBank),
}

impl QueueResponse {
    /// Command that owns this closed response shape.
    pub const fn command(self) -> QueueCommand {
        match self {
            Self::Activate(_) => QueueCommand::Activate,
            Self::ReadStatus(_) => QueueCommand::ReadStatus,
            Self::PublishSqProducer(_) => QueueCommand::PublishSqProducer,
            Self::CaptureSqProducer(_) => QueueCommand::CaptureSqProducer,
            Self::PublishSqConsumer(_) => QueueCommand::PublishSqConsumer,
            Self::PublishCqProducer(_) => QueueCommand::PublishCqProducer,
            Self::PublishCqConsumer(_) => QueueCommand::PublishCqConsumer,
            Self::PublishEqProducer(_) => QueueCommand::PublishEqProducer,
            Self::PublishEqConsumer(_) => QueueCommand::PublishEqConsumer,
        }
    }

    /// Complete named queue response bank published on success.
    pub const fn bank(self) -> QueueResponseBank {
        match self {
            Self::Activate(bank)
            | Self::ReadStatus(bank)
            | Self::PublishSqProducer(bank)
            | Self::CaptureSqProducer(bank)
            | Self::PublishSqConsumer(bank)
            | Self::PublishCqProducer(bank)
            | Self::PublishCqConsumer(bank)
            | Self::PublishEqProducer(bank)
            | Self::PublishEqConsumer(bank) => bank,
        }
    }
}

/// Queue response-bank effect of a structurally valid command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueResponseUpdate {
    /// Successful command response replaces all named fields.
    Success(QueueResponse),
    /// Semantic failure preserves the preceding bank.
    Preserve,
}

/// Applies the queue response-bank success or failure rule.
pub const fn apply_queue_response_update(
    previous: QueueResponseBank,
    update: QueueResponseUpdate,
) -> QueueResponseBank {
    match update {
        QueueResponseUpdate::Success(response) => response.bank(),
        QueueResponseUpdate::Preserve => previous,
    }
}
