mod global;
mod irq;
mod queue;

/// Checked generated response-bank encoding failure.
#[derive(Debug)]
pub enum ResponseCodecError {
    /// The generated map view rejected the bank buffer.
    Map(hardware_interface_schema::SchemaCodecError),
    /// A generated register value view rejected a value.
    Register(hardware_interface_schema::RegisterCodecError),
}

impl core::fmt::Display for ResponseCodecError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Map(error) => write!(formatter, "response map view failed: {error}"),
            Self::Register(error) => write!(formatter, "response register view failed: {error}"),
        }
    }
}

impl std::error::Error for ResponseCodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Map(source) => Some(source),
            Self::Register(source) => Some(source),
        }
    }
}

impl From<hardware_interface_schema::SchemaCodecError> for ResponseCodecError {
    fn from(error: hardware_interface_schema::SchemaCodecError) -> Self {
        Self::Map(error)
    }
}

impl From<hardware_interface_schema::RegisterCodecError> for ResponseCodecError {
    fn from(error: hardware_interface_schema::RegisterCodecError) -> Self {
        Self::Register(error)
    }
}

pub use global::{
    GlobalLimitsResponse, GlobalResetResponse, GlobalResponse, GlobalResponseBank,
    GlobalResponseRegisters, GlobalResponseRegistersRegisterId, GlobalResponseRegistersView,
    GlobalResponseRegistersViewMut, GlobalResponseUpdate, GlobalStatusResponse,
    apply_global_response_update,
};
pub use irq::{
    IrqClaimNotificationResponse, IrqLineSnapshotResponse, IrqMessageNotificationResponse,
    IrqResponse, IrqResponseBank, IrqResponseRegisters, IrqResponseRegistersRegisterId,
    IrqResponseRegistersView, IrqResponseRegistersViewMut, IrqResponseUpdate, IrqSourceResponse,
    apply_irq_response_update,
};
pub use queue::{
    QueueResponse, QueueResponseBank, QueueResponseRegisters, QueueResponseRegistersRegisterId,
    QueueResponseRegistersView, QueueResponseRegistersViewMut, QueueResponseUpdate,
    apply_queue_response_update,
};
