use crate::abi::mmio::register::irq::{
    IRspEpochValues, IRspMessageIdValues, IRspNotificationKindValues, IRspPendingMessagesValues,
    IRspRouteKindValues, IRspSourceIdValues, IRspSourceKindValues, IRspStateValues,
    IRspVectorValues, IrqResponseRegistersValues,
};
pub use crate::abi::mmio::register::irq::{
    IrqResponseRegisters, IrqResponseRegistersRegisterId, IrqResponseRegistersView,
    IrqResponseRegistersViewMut,
};
use crate::abi::mmio::{IrqCommand, IrqSourceStatus};

/// Exact bytes published through the IRQ response registers.
pub type IrqResponseBank = IrqResponseRegistersValues;

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
        Self::ZERO
    }

    const fn source(response: IrqSourceResponse) -> Self {
        Self {
            epoch: IRspEpochValues::new(response.epoch),
            source_id: IRspSourceIdValues::new(response.source_id),
            source_kind: IRspSourceKindValues::new(response.source_kind),
            state: IRspStateValues::new(response.state.bits()),
            route_kind: IRspRouteKindValues::new(response.route_kind),
            notification_kind: IRspNotificationKindValues::new(0),
            vector: IRspVectorValues::new(response.vector),
            message_id: IRspMessageIdValues::new(0),
            pending_messages: IRspPendingMessagesValues::new(response.pending_messages),
        }
    }

    const fn line(response: IrqLineSnapshotResponse) -> Self {
        Self {
            epoch: IRspEpochValues::new(response.epoch),
            source_id: IRspSourceIdValues::new(response.source_id),
            source_kind: IRspSourceKindValues::new(response.source_kind),
            state: IRspStateValues::new(response.state.bits()),
            route_kind: IRspRouteKindValues::new(response.route_kind),
            notification_kind: IRspNotificationKindValues::new(1),
            vector: IRspVectorValues::new(response.vector),
            message_id: IRspMessageIdValues::new(0),
            pending_messages: IRspPendingMessagesValues::new(0),
        }
    }

    const fn message(response: IrqMessageNotificationResponse) -> Self {
        Self {
            epoch: IRspEpochValues::new(response.epoch),
            source_id: IRspSourceIdValues::new(response.source_id),
            source_kind: IRspSourceKindValues::new(response.source_kind),
            state: IRspStateValues::new(response.state.bits()),
            route_kind: IRspRouteKindValues::new(response.route_kind),
            notification_kind: IRspNotificationKindValues::new(2),
            vector: IRspVectorValues::new(response.vector),
            message_id: IRspMessageIdValues::new(response.message_id),
            pending_messages: IRspPendingMessagesValues::new(response.pending_messages),
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
