//! Stable, fixed-width PDMA data ABI vocabulary.

mod address;
mod codec;
mod descriptor;
mod error;
mod metadata;

pub use address::{AbiId, DmaAddress, DmaRegion};
pub use codec::{copy_u64_le, read_u16_le, read_u64_le, write_u16_le, write_u64_le};
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
    INDIRECT_ENTRY_ALIGNMENT, IOVA64_FEATURE, KNOWN_FEATURE_MASK, MAX_INDIRECT_DESTINATIONS,
    MAX_RING_DEPTH, MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES, NamedValue, OPCODES,
    OpcodeSpec, RecordSpec, STRIDED_COPY_FEATURE, STRIDED_COPY_OPCODE, SUCCESS_STATUS,
    VECTOR_ADD_FEATURE, VECTOR_ADD_OPCODE, validate_abi_version,
};
