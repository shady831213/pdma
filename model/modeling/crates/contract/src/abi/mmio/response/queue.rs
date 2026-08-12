use hardware_interface_schema::{RegisterCodecError, register_map};

use super::ResponseCodecError;
use crate::abi::mmio::{QueueCommand, queue};

register_map! { pub map QueueResponseRegisters {
    size: 72,
    fixed: {
        epoch: queue::QRspEpoch { id: 0, name: "Q_RSP_EPOCH", offset: 0 },
        queue_id: queue::QRspId { id: 1, name: "Q_RSP_ID", offset: 8 },
        sq_producer: queue::QRspSqProducer { id: 2, name: "Q_RSP_SQ_PRODUCER", offset: 16 },
        sq_captured: queue::QRspSqCaptured { id: 3, name: "Q_RSP_SQ_CAPTURED", offset: 24 },
        sq_consumer: queue::QRspSqConsumer { id: 4, name: "Q_RSP_SQ_CONSUMER", offset: 32 },
        cq_producer: queue::QRspCqProducer { id: 5, name: "Q_RSP_CQ_PRODUCER", offset: 40 },
        cq_consumer: queue::QRspCqConsumer { id: 6, name: "Q_RSP_CQ_CONSUMER", offset: 48 },
        eq_producer: queue::QRspEqProducer { id: 7, name: "Q_RSP_EQ_PRODUCER", offset: 56 },
        eq_consumer: queue::QRspEqConsumer { id: 8, name: "Q_RSP_EQ_CONSUMER", offset: 64 },
    },
    banks: {}
} }

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
    pub fn to_le_bytes(self) -> Result<[u8; 72], ResponseCodecError> {
        let mut bytes = [0; 72];
        let mut view = QueueResponseRegistersViewMut::new(&mut bytes)?;
        self.write_to(&mut view)?;
        Ok(bytes)
    }

    fn write_to(
        self,
        view: &mut QueueResponseRegistersViewMut<'_>,
    ) -> Result<(), RegisterCodecError> {
        view.epoch_mut()?.set_value(self.epoch)?;
        view.queue_id_mut()?.set_value(self.queue_id)?;
        view.sq_producer_mut()?.set_value(self.sq_producer)?;
        view.sq_captured_mut()?.set_value(self.sq_captured)?;
        view.sq_consumer_mut()?.set_value(self.sq_consumer)?;
        view.cq_producer_mut()?.set_value(self.cq_producer)?;
        view.cq_consumer_mut()?.set_value(self.cq_consumer)?;
        view.eq_producer_mut()?.set_value(self.eq_producer)?;
        view.eq_consumer_mut()?.set_value(self.eq_consumer)
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
