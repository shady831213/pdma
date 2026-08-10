//! Stable, fixed-width PDMA data ABI vocabulary.

mod address;
mod codec;
mod control;
mod descriptor;
mod error;
pub mod header;
mod metadata;
pub mod mmio;
mod record;
mod ring;
mod schema;

pub use address::{AbiId, DmaAddress, DmaRegion};
pub use codec::{copy_u64_le, read_u16_le, read_u64_le, write_u16_le, write_u64_le};
pub use control::{
    AbiDiscovery, AbiLimits, ControlRequest, DeviceEpoch, DeviceRunState, DeviceStatus,
    DiscoveryRequest, EpochIrqRoute, EpochIrqSource, EpochQueue, IrqControl, QueueActivation,
    QueueControl, QueueCursorUpdate, QueueRingRegions, WholeDeviceReset,
};
pub use descriptor::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiStridedCopy, AbiVectorAdd, BroadcastDestinationEntry,
    BroadcastIovas, BroadcastShape, CopyIovas, CopyStrides, DescriptorDecodeError,
    DescriptorValueError, StridedCopyShape, VectorIovas,
};
pub use error::{AbiAddressError, AbiControlError, AbiDecodeError, AbiEncodeError};
pub use metadata::{
    ABI_VERSION, AbiV1Features, BROADCAST_COPY_OPCODE, COMPLETION, COMPLETION_ALIGNMENT,
    COMPLETION_FLAGS_MASK, COMPLETION_SIZE, COMPLETION_V1_FEATURE, COPY_FEATURE, COPY_OPCODE,
    DESCRIPTOR, DESCRIPTOR_ALIGNMENT, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DESCRIPTOR_V1_FEATURE, EVENT, EVENT_ACCESSES, EVENT_ALIGNMENT, EVENT_FLAGS_MASK, EVENT_KINDS,
    EVENT_SIZE, EVENT_STAGES, EVENT_V1_FEATURE, FEATURES, FieldSpec, INDIRECT_BROADCAST_FEATURE,
    INDIRECT_DESTINATION_ENTRY, INDIRECT_DESTINATION_ENTRY_SIZE, INDIRECT_ENTRY_ALIGNMENT,
    IOVA64_FEATURE, KNOWN_FEATURE_MASK, MAX_INDIRECT_DESTINATIONS, MAX_RING_DEPTH, MAX_STRIDE,
    MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES, NamedValue, OPCODES, OpcodeSpec, RecordSpec,
    STRIDED_COPY_FEATURE, STRIDED_COPY_OPCODE, SUCCESS_STATUS, VECTOR_ADD_FEATURE,
    VECTOR_ADD_OPCODE, validate_abi_version,
};
pub use record::{
    AbiBounds, AbiEvent, AbiEventAccess, AbiEventKind, AbiEventStage, AbiFault, AbiFaultCandidate,
    AbiFaultDetails, AbiFaultOrder, AbiFaultSelectionError, AbiFaultSite, AbiHeaderField,
    AbiOperandFaultStep, AbiOperandVisit, AbiReservedField, AbiScalarField, AbiStrideOffset,
    decode_completion, decode_event, encode_completion, encode_event, encode_semantic_event,
    select_canonical_fault,
};
pub use ring::{RingCursor, RingCursors, RingLayout, RingRecord};
pub use schema::{
    CompletionSchema, CompletionSchemaView, DescriptorSchema, DescriptorSchemaView, EventSchema,
    EventSchemaView, IndirectDestinationEntrySchema, IndirectDestinationEntrySchemaView,
};
