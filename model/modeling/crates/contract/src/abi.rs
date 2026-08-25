//! Stable, fixed-width PDMA data ABI vocabulary.

mod address;
mod codec;
mod descriptor;
mod error;
pub mod header;
mod metadata;
pub mod mmio;
mod record;
mod ring;

pub use address::{AbiId, DmaAddress};
pub use codec::copy_u64_le;
pub use descriptor::{
    AbiCommand, BroadcastDestinationEntry, DescriptorDecodeError, DescriptorHeaderSchema,
    DescriptorHeaderSchemaFlagsRaw, DescriptorHeaderSchemaFlagsValues,
    DescriptorHeaderSchemaOpcode, DescriptorHeaderSchemaOpcodeRaw, DescriptorHeaderSchemaRaw,
    DescriptorHeaderSchemaValues, DescriptorHeaderSchemaView, DescriptorHeaderSchemaViewMut,
    DescriptorSchema, DescriptorSchemaPayloadBroadcastCopyValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadRaw,
    DescriptorSchemaPayloadStridedCopyValues, DescriptorSchemaPayloadValues,
    DescriptorSchemaPayloadVectorAddValues, DescriptorSchemaRaw, DescriptorSchemaValues,
    DescriptorSchemaView, DescriptorSchemaViewMut, DescriptorValueError,
    IndirectDestinationEntrySchema, IndirectDestinationEntrySchemaRaw,
    IndirectDestinationEntrySchemaValues, IndirectDestinationEntrySchemaView,
    IndirectDestinationEntrySchemaViewMut,
};
pub use error::{AbiAddressError, AbiControlError, AbiDecodeError, AbiEncodeError};
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
    AbiBounds, AbiCompletion, AbiEvent as ExactAbiEvent, AbiEventAccess, AbiEventKind,
    AbiEventStage, AbiFault, AbiFaultCandidate, AbiFaultDetails, AbiFaultOrder,
    AbiFaultSelectionError, AbiFaultSite, AbiHeaderField, AbiOperandFaultStep, AbiOperandVisit,
    AbiReservedField, AbiScalarField, AbiStrideOffset, CompletionSchema, CompletionSchemaValues,
    CompletionSchemaView, CompletionSchemaViewMut, EventSchema, EventSchemaValues, EventSchemaView,
    EventSchemaViewMut, decode_completion as decode_exact_completion,
    decode_event as decode_exact_event, encode_completion as encode_exact_completion,
    encode_event as encode_exact_event, select_canonical_fault,
};
pub use ring::{RingCursor, RingCursors, RingLayout, RingRecord};
