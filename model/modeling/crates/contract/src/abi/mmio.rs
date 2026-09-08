//! Exact 4 KiB numeric MMIO ABI vocabulary and pure codecs.

mod access;
mod codec;
mod error;
mod register;
mod response;
mod result;
mod status;

pub use access::{decode_read, decode_write, register_at};
pub use codec::{decode_u32, decode_u64, encode_u32, encode_u64};
pub use error::MmioAccessError;
pub use register::global::{
    GlobalRegisters, GlobalRegistersRaw, GlobalRegistersRegisterId, GlobalRegistersValues,
    GlobalRegistersView, GlobalRegistersViewMut,
};
pub use register::irq::{
    IrqRegisters, IrqRegistersRaw, IrqRegistersRegisterId, IrqRegistersValues, IrqRegistersView,
    IrqRegistersViewMut,
};
pub use register::queue::QueueResponseRegistersRaw;
pub use register::queue::{
    QueueRegisters, QueueRegistersRaw, QueueRegistersRegisterId, QueueRegistersValues,
    QueueRegistersView, QueueRegistersViewMut,
};
pub use register::{
    MMIO_REGION_SIZE, MmioAccess, MmioByteCount, MmioReset, PdmaMmioMap, PdmaMmioMapRaw,
    PdmaMmioMapRegisterId, PdmaMmioMapValues, PdmaMmioMapView, PdmaMmioMapViewMut, RegisterSpec,
    global, irq, queue,
};
pub use response::{
    GlobalLimitsResponse, GlobalResetResponse, GlobalResponse, GlobalResponseBank,
    GlobalResponseRegisters, GlobalResponseRegistersRegisterId, GlobalResponseRegistersView,
    GlobalResponseRegistersViewMut, GlobalResponseUpdate, GlobalStatusResponse,
    IrqClaimNotificationResponse, IrqLineSnapshotResponse, IrqMessageNotificationResponse,
    IrqResponse, IrqResponseBank, IrqResponseRegisters, IrqResponseRegistersRegisterId,
    IrqResponseRegistersView, IrqResponseRegistersViewMut, IrqResponseUpdate, IrqSourceResponse,
    QueueResponse, QueueResponseBank, QueueResponseRegisters, QueueResponseRegistersRegisterId,
    QueueResponseRegistersView, QueueResponseRegistersViewMut, QueueResponseUpdate,
    ResponseCodecError, apply_global_response_update, apply_irq_response_update,
    apply_queue_response_update,
};
pub use result::{
    GlobalCommand, IrqCommand, MmioResult, MmioResultDetails, QueueCommand, ResponseUpdate,
    UnknownCommand, UnknownResult, apply_response_update,
};
pub use status::{IrqSourceStatus, IrqSourceStatusError, IrqStateBit};
