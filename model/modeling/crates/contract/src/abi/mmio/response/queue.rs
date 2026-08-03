use crate::abi::mmio::QueueCommand;

/// Exact typed queue response register bank.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueueResponseBank {
    /// Current device epoch.
    pub epoch: u64,
    /// Numeric queue identity.
    pub queue_id: u64,
    /// Submission producer cursor.
    pub sq_producer: u64,
    /// Captured submission producer cursor.
    pub sq_captured: u64,
    /// Submission consumer cursor.
    pub sq_consumer: u64,
    /// Completion producer cursor.
    pub cq_producer: u64,
    /// Completion consumer cursor.
    pub cq_consumer: u64,
    /// Event producer cursor.
    pub eq_producer: u64,
    /// Event consumer cursor.
    pub eq_consumer: u64,
}

impl QueueResponseBank {
    /// Exact contiguous little-endian response-register bytes.
    pub fn to_le_bytes(self) -> [u8; 72] {
        let mut bytes = [0; 72];
        let values = self
            .epoch
            .to_le_bytes()
            .into_iter()
            .chain(self.queue_id.to_le_bytes())
            .chain(self.sq_producer.to_le_bytes())
            .chain(self.sq_captured.to_le_bytes())
            .chain(self.sq_consumer.to_le_bytes())
            .chain(self.cq_producer.to_le_bytes())
            .chain(self.cq_consumer.to_le_bytes())
            .chain(self.eq_producer.to_le_bytes())
            .chain(self.eq_consumer.to_le_bytes());
        for (destination, source) in bytes.iter_mut().zip(values) {
            *destination = source;
        }
        bytes
    }
}

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
