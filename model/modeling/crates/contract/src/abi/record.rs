//! Fixed-width completion and event record codecs.

mod conversion;
mod schema;

pub use schema::{
    AbiCompletion, AbiCompletionFlagsRaw, AbiCompletionFlagsValues, AbiCompletionRaw,
    AbiCompletionReservedRaw, AbiCompletionReservedValues, AbiCompletionStatus,
    AbiCompletionStatusRaw, AbiCompletionView, AbiCompletionViewMut, AbiEvent, AbiEventAccess,
    AbiEventAccessRaw, AbiEventFlagsRaw, AbiEventFlagsValues, AbiEventKind, AbiEventKindRaw,
    AbiEventRaw, AbiEventReservedRaw, AbiEventReservedValues, AbiEventStage, AbiEventStageRaw,
    AbiEventView, AbiEventViewMut,
};

const _: () = assert!(AbiCompletion::RECORD_SIZE == 32);
const _: () = assert!(AbiEvent::RECORD_SIZE == 64);
