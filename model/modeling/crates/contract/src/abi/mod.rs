//! Stable, fixed-width PDMA data ABI vocabulary.

mod address;
mod codec;
mod control;
mod descriptor;
mod error;
mod fault;
pub mod header;
mod metadata;
pub mod mmio;
pub mod record;
mod ring;

pub use address::DmaAddress;
pub use control::{AbiDiscovery, AbiLimits, DiscoveryRequest};
pub use descriptor::{
    AbiCommand, AbiCommandHeader, AbiCommandHeaderFlagsRaw, AbiCommandHeaderFlagsValues,
    AbiCommandHeaderOpcode, AbiCommandHeaderOpcodeRaw, AbiCommandHeaderRaw, AbiCommandHeaderView,
    AbiCommandHeaderViewMut, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadRaw, AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues,
    AbiCommandPayloadVectorAddValues, AbiCommandRaw, AbiCommandView, AbiCommandViewMut,
    BroadcastDestinationEntry, DescriptorDecodeError, DescriptorViolation, DescriptorViolationKind,
    IndirectDestinationEntrySchema, IndirectDestinationEntrySchemaRaw,
    IndirectDestinationEntrySchemaView, IndirectDestinationEntrySchemaViewMut, U32Word, U32WordRaw,
    U32WordValues,
};
pub use error::{AbiAddressError, AbiControlError, AbiDecodeError, AbiEncodeError};
pub use fault::{
    AbiBounds, AbiEventAccess, AbiEventKind, AbiEventStage, AbiFault, AbiFaultCandidate,
    AbiFaultDetails, AbiFaultOrder, AbiFaultSelectionError, AbiFaultSite, AbiOperandFaultStep,
    AbiOperandVisit, AbiStrideOffset, select_canonical_fault,
};
pub use metadata::{
    ABI_VERSION, AbiV1Features, BROADCAST_COPY_OPCODE, COMPLETION, COMPLETION_ALIGNMENT,
    COMPLETION_FLAGS_MASK, COMPLETION_SIZE, COMPLETION_V1_FEATURE, COPY_FEATURE, COPY_OPCODE,
    DESCRIPTOR, DESCRIPTOR_ALIGNMENT, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DESCRIPTOR_V1_FEATURE, DMA_ADDRESS_64_FEATURE, EVENT, EVENT_ACCESSES, EVENT_ALIGNMENT,
    EVENT_FLAGS_MASK, EVENT_KINDS, EVENT_SIZE, EVENT_STAGES, EVENT_V1_FEATURE, FEATURES, FieldSpec,
    INDIRECT_BROADCAST_FEATURE, INDIRECT_DESTINATION_ENTRY, INDIRECT_DESTINATION_ENTRY_SIZE,
    INDIRECT_ENTRY_ALIGNMENT, KNOWN_FEATURE_MASK, MAX_INDIRECT_DESTINATIONS, MAX_RING_DEPTH,
    MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES, NamedValue, OPCODES, OpcodeSpec, RecordSpec,
    STRIDED_COPY_FEATURE, STRIDED_COPY_OPCODE, SUCCESS_STATUS, VECTOR_ADD_FEATURE,
    VECTOR_ADD_OPCODE, validate_abi_version,
};
pub use record::{
    AbiCompletion, AbiCompletionFlagsRaw, AbiCompletionFlagsValues, AbiCompletionRaw,
    AbiCompletionReservedRaw, AbiCompletionReservedValues, AbiCompletionStatus,
    AbiCompletionStatusRaw, AbiCompletionView, AbiCompletionViewMut, AbiEvent, AbiEventAccessRaw,
    AbiEventFlagsRaw, AbiEventFlagsValues, AbiEventKindRaw, AbiEventRaw, AbiEventReservedRaw,
    AbiEventReservedValues, AbiEventStageRaw, AbiEventView, AbiEventViewMut,
};
pub use ring::{RingCursor, RingRecord};
