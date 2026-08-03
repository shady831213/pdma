use crate::abi::mmio::{IrqCommand, IrqSourceStatus};

/// Exact bytes published through the IRQ response registers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrqResponseBank {
    epoch: u64,
    source_id: u64,
    source_kind: u32,
    state: u32,
    route_kind: u32,
    notification_kind: u32,
    vector: u64,
    message_id: u64,
    pending_messages: u64,
}

impl IrqResponseBank {
    /// Exact contiguous little-endian response-register bytes.
    pub fn to_le_bytes(self) -> [u8; 56] {
        let mut bytes = [0; 56];
        let values = self
            .epoch
            .to_le_bytes()
            .into_iter()
            .chain(self.source_id.to_le_bytes())
            .chain(self.source_kind.to_le_bytes())
            .chain(self.state.to_le_bytes())
            .chain(self.route_kind.to_le_bytes())
            .chain(self.notification_kind.to_le_bytes())
            .chain(self.vector.to_le_bytes())
            .chain(self.message_id.to_le_bytes())
            .chain(self.pending_messages.to_le_bytes());
        for (destination, source) in bytes.iter_mut().zip(values) {
            *destination = source;
        }
        bytes
    }
}

/// Source state returned by the source-scoped IRQ commands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrqSourceResponse {
    /// Current device epoch.
    pub epoch: u64,
    /// Numeric PDMA-local source identity.
    pub source_id: u64,
    /// Numeric source kind.
    pub source_kind: u32,
    /// Complete checked source state bits.
    pub state: IrqSourceStatus,
    /// Numeric route kind.
    pub route_kind: u32,
    /// Route vector.
    pub vector: u64,
    /// Pending message count.
    pub pending_messages: u64,
}

/// Line snapshot returned by `SNAPSHOT_LINE`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrqLineSnapshotResponse {
    /// Current device epoch.
    pub epoch: u64,
    /// Numeric PDMA-local source identity.
    pub source_id: u64,
    /// Numeric source kind.
    pub source_kind: u32,
    /// Complete checked source state bits.
    pub state: IrqSourceStatus,
    /// Numeric route kind.
    pub route_kind: u32,
    /// Line vector.
    pub vector: u64,
}

/// Message notification returned by `CLAIM_NOTIFICATION`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrqMessageNotificationResponse {
    /// Current device epoch.
    pub epoch: u64,
    /// Numeric PDMA-local source identity.
    pub source_id: u64,
    /// Numeric source kind.
    pub source_kind: u32,
    /// Post-claim checked source state bits.
    pub state: IrqSourceStatus,
    /// Numeric Message route kind.
    pub route_kind: u32,
    /// Message vector.
    pub vector: u64,
    /// Source-scoped message identity.
    pub message_id: u64,
    /// Pending message count after claim.
    pub pending_messages: u64,
}

/// Typed notification returned by `CLAIM_NOTIFICATION`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrqClaimNotificationResponse {
    /// A line notification.
    Line(IrqLineSnapshotResponse),
    /// A message notification.
    Message(IrqMessageNotificationResponse),
}

/// Closed successful IRQ command responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrqResponse {
    /// `CONFIGURE_ROUTE` returns complete source state.
    ConfigureRoute(IrqSourceResponse),
    /// `ENABLE` returns complete source state.
    Enable(IrqSourceResponse),
    /// `DISABLE` returns complete source state.
    Disable(IrqSourceResponse),
    /// `MASK` returns complete source state.
    Mask(IrqSourceResponse),
    /// `UNMASK` returns complete source state.
    Unmask(IrqSourceResponse),
    /// `ACKNOWLEDGE` returns complete source state.
    Acknowledge(IrqSourceResponse),
    /// `OBSERVE_SOURCE` returns complete source state.
    ObserveSource(IrqSourceResponse),
    /// `SNAPSHOT_LINE` returns an asserted line observation.
    SnapshotLine(IrqLineSnapshotResponse),
    /// `SNAPSHOT_LINE` returned no line; all fields are zero.
    SnapshotLineEmpty,
    /// `CLAIM_NOTIFICATION` returns a line or message notification.
    ClaimNotification(IrqClaimNotificationResponse),
    /// `CLAIM_NOTIFICATION` returned no notification; all fields are zero.
    ClaimNotificationEmpty,
}

impl IrqResponse {
    /// Command that owns this closed response shape.
    pub const fn command(self) -> IrqCommand {
        match self {
            Self::ConfigureRoute(_) => IrqCommand::ConfigureRoute,
            Self::Enable(_) => IrqCommand::Enable,
            Self::Disable(_) => IrqCommand::Disable,
            Self::Mask(_) => IrqCommand::Mask,
            Self::Unmask(_) => IrqCommand::Unmask,
            Self::Acknowledge(_) => IrqCommand::Acknowledge,
            Self::ObserveSource(_) => IrqCommand::ObserveSource,
            Self::SnapshotLine(_) | Self::SnapshotLineEmpty => IrqCommand::SnapshotLine,
            Self::ClaimNotification(_) | Self::ClaimNotificationEmpty => {
                IrqCommand::ClaimNotification
            }
        }
    }

    /// Populates the exact response bank, zero-filling irrelevant fields.
    pub const fn bank(self) -> IrqResponseBank {
        match self {
            Self::ConfigureRoute(response)
            | Self::Enable(response)
            | Self::Disable(response)
            | Self::Mask(response)
            | Self::Unmask(response)
            | Self::Acknowledge(response)
            | Self::ObserveSource(response) => IrqResponseBank::source(response),
            Self::SnapshotLine(response) => IrqResponseBank::line(response),
            Self::SnapshotLineEmpty | Self::ClaimNotificationEmpty => IrqResponseBank::zero(),
            Self::ClaimNotification(IrqClaimNotificationResponse::Line(response)) => {
                IrqResponseBank::line(response)
            }
            Self::ClaimNotification(IrqClaimNotificationResponse::Message(response)) => {
                IrqResponseBank::message(response)
            }
        }
    }
}

impl IrqResponseBank {
    const fn zero() -> Self {
        Self {
            epoch: 0,
            source_id: 0,
            source_kind: 0,
            state: 0,
            route_kind: 0,
            notification_kind: 0,
            vector: 0,
            message_id: 0,
            pending_messages: 0,
        }
    }

    const fn source(response: IrqSourceResponse) -> Self {
        Self {
            epoch: response.epoch,
            source_id: response.source_id,
            source_kind: response.source_kind,
            state: response.state.bits(),
            route_kind: response.route_kind,
            notification_kind: 0,
            vector: response.vector,
            message_id: 0,
            pending_messages: response.pending_messages,
        }
    }

    const fn line(response: IrqLineSnapshotResponse) -> Self {
        Self {
            epoch: response.epoch,
            source_id: response.source_id,
            source_kind: response.source_kind,
            state: response.state.bits(),
            route_kind: response.route_kind,
            notification_kind: 1,
            vector: response.vector,
            message_id: 0,
            pending_messages: 0,
        }
    }

    const fn message(response: IrqMessageNotificationResponse) -> Self {
        Self {
            epoch: response.epoch,
            source_id: response.source_id,
            source_kind: response.source_kind,
            state: response.state.bits(),
            route_kind: response.route_kind,
            notification_kind: 2,
            vector: response.vector,
            message_id: response.message_id,
            pending_messages: response.pending_messages,
        }
    }
}

/// IRQ response-bank effect of a structurally valid command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrqResponseUpdate {
    /// Successful command response replaces the bank.
    Success(IrqResponse),
    /// Semantic failure preserves the preceding bank.
    Preserve,
}

/// Applies the IRQ response-bank success or failure rule.
pub const fn apply_irq_response_update(
    previous: IrqResponseBank,
    update: IrqResponseUpdate,
) -> IrqResponseBank {
    match update {
        IrqResponseUpdate::Success(response) => response.bank(),
        IrqResponseUpdate::Preserve => previous,
    }
}
