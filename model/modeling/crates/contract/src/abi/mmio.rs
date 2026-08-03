//! Exact 4 KiB numeric MMIO ABI vocabulary and pure codecs.

mod access;
mod codec;
mod error;
mod register;
mod response;
mod result;
mod status;

pub use access::{decode_read, decode_write};
pub use codec::{decode_u32, decode_u64, encode_u32, encode_u64};
pub use error::MmioAccessError;
pub use register::{
    MMIO_REGION_SIZE, MMIO_REGISTERS, MmioAccess, MmioByteCount, MmioReset, RegisterId,
    RegisterRegistry, RegisterSpec,
};
pub use response::{
    GlobalLimitsResponse, GlobalResetResponse, GlobalResponse, GlobalResponseBank,
    GlobalResponseUpdate, GlobalStatusResponse, IrqClaimNotificationResponse,
    IrqLineSnapshotResponse, IrqMessageNotificationResponse, IrqResponse, IrqResponseBank,
    IrqResponseUpdate, IrqSourceResponse, QueueResponse, QueueResponseBank, QueueResponseUpdate,
    apply_global_response_update, apply_irq_response_update, apply_queue_response_update,
};
pub use result::{
    GlobalCommand, IrqCommand, MmioResult, MmioResultDetails, QueueCommand, ResponseUpdate,
    UnknownCommand, UnknownResult, apply_response_update,
};
pub use status::{IrqSourceStatus, IrqSourceStatusError, IrqStateBit};
