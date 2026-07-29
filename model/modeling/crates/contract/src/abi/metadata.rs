//! Canonical numeric registry for PDMA ABI version 1.

use core::ops::Range;

use super::{AbiControlError, AbiDecodeError};

/// ABI version implemented by this registry.
pub const ABI_VERSION: u16 = 1;

/// Descriptor record width in bytes.
pub const DESCRIPTOR_SIZE: u16 = 64;
/// Completion record width in bytes.
pub const COMPLETION_SIZE: u16 = 32;
/// Event record width in bytes.
pub const EVENT_SIZE: u16 = 64;

/// Required descriptor base alignment.
pub const DESCRIPTOR_ALIGNMENT: u64 = 64;
/// Required completion-ring base alignment.
pub const COMPLETION_ALIGNMENT: u64 = 32;
/// Required event-ring base alignment.
pub const EVENT_ALIGNMENT: u64 = 64;
/// Required indirect destination-entry alignment.
pub const INDIRECT_ENTRY_ALIGNMENT: u64 = 8;

/// Maximum representable ring depth.
pub const MAX_RING_DEPTH: u32 = u32::MAX;
/// Maximum representable indirect destination count.
pub const MAX_INDIRECT_DESTINATIONS: u32 = u32::MAX;
/// Maximum representable transfer length.
pub const MAX_TRANSFER_BYTES: u32 = u32::MAX;
/// Maximum representable strided row count.
pub const MAX_STRIDED_ROWS: u32 = u32::MAX;
/// Maximum representable stride.
pub const MAX_STRIDE: u32 = u32::MAX;

/// IOVA64 discovery feature.
pub const IOVA64_FEATURE: u64 = 1 << 0;
/// Descriptor-v1 discovery feature.
pub const DESCRIPTOR_V1_FEATURE: u64 = 1 << 1;
/// Completion-v1 discovery feature.
pub const COMPLETION_V1_FEATURE: u64 = 1 << 2;
/// Event-v1 discovery feature.
pub const EVENT_V1_FEATURE: u64 = 1 << 3;
/// Copy operation discovery feature.
pub const COPY_FEATURE: u64 = 1 << 4;
/// Strided-copy operation discovery feature.
pub const STRIDED_COPY_FEATURE: u64 = 1 << 5;
/// Indirect-broadcast operation discovery feature.
pub const INDIRECT_BROADCAST_FEATURE: u64 = 1 << 6;
/// Vector-add operation discovery feature.
pub const VECTOR_ADD_FEATURE: u64 = 1 << 7;
/// Complete feature mask assigned by ABI v1.
pub const KNOWN_FEATURE_MASK: u64 = 0xff;

/// Validates a requested ABI version at the typed control boundary.
pub const fn validate_abi_version(version: u16) -> Result<(), AbiControlError> {
    if version == ABI_VERSION {
        Ok(())
    } else {
        Err(AbiControlError::UnsupportedVersion { requested: version })
    }
}

/// Copy descriptor opcode.
pub const COPY_OPCODE: u16 = 1;
/// Strided-copy descriptor opcode.
pub const STRIDED_COPY_OPCODE: u16 = 2;
/// Broadcast-copy descriptor opcode.
pub const BROADCAST_COPY_OPCODE: u16 = 3;
/// Vector-add descriptor opcode.
pub const VECTOR_ADD_OPCODE: u16 = 4;

/// Descriptor flags accepted by ABI v1.
pub const DESCRIPTOR_FLAGS_MASK: u16 = 0;
/// Completion flags accepted by ABI v1.
pub const COMPLETION_FLAGS_MASK: u16 = 0;
/// Event flags accepted by ABI v1.
pub const EVENT_FLAGS_MASK: u16 = 0;
/// Successful completion status.
pub const SUCCESS_STATUS: u16 = 0;

/// An assigned numeric value with a stable generator-facing name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedValue {
    /// Symbolic ABI name.
    pub name: &'static str,
    /// Assigned fixed-width value.
    pub value: u16,
}

impl NamedValue {
    const fn new(name: &'static str, value: u16) -> Self {
        Self { name, value }
    }
}

/// An assigned descriptor opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcodeSpec {
    name: &'static str,
    value: u16,
}

impl OpcodeSpec {
    /// Symbolic opcode name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Assigned opcode value.
    pub const fn value(self) -> u16 {
        self.value
    }
}

/// One fixed byte range in a canonical record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    name: &'static str,
    offset: u16,
    width: u16,
}

impl FieldSpec {
    const fn new(name: &'static str, offset: u16, width: u16) -> Self {
        Self {
            name,
            offset,
            width,
        }
    }

    /// Generator-facing field name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Starting byte offset.
    pub const fn offset(self) -> u16 {
        self.offset
    }

    /// Fixed field width.
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Exclusive byte end.
    pub const fn end(self) -> u16 {
        self.offset + self.width
    }

    /// Byte range for checked codec access.
    pub fn range(self) -> Range<usize> {
        usize::from(self.offset)..usize::from(self.end())
    }
}

/// Canonical record size, alignment, and ordered field table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordSpec {
    size: u16,
    alignment: u64,
    fields: &'static [FieldSpec],
}

impl RecordSpec {
    /// Encoded record width.
    pub const fn size(self) -> u16 {
        self.size
    }

    /// Required ring-base alignment.
    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    /// Ordered canonical fields.
    pub const fn fields(self) -> &'static [FieldSpec] {
        self.fields
    }
}

/// Validated ABI-v1 discovery features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiV1Features(u64);

impl AbiV1Features {
    /// Returns the validated fixed-width feature mask.
    pub const fn bits(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for AbiV1Features {
    type Error = AbiDecodeError;

    fn try_from(bits: u64) -> Result<Self, Self::Error> {
        let unknown = bits & !KNOWN_FEATURE_MASK;
        if unknown == 0 {
            Ok(Self(bits))
        } else {
            Err(AbiDecodeError::UnknownFeatureBits { bits: unknown })
        }
    }
}

/// ABI-v1 discovery feature assignments.
pub const FEATURES: [(&str, u64); 8] = [
    ("IOVA64", IOVA64_FEATURE),
    ("DESCRIPTOR_V1", DESCRIPTOR_V1_FEATURE),
    ("COMPLETION_V1", COMPLETION_V1_FEATURE),
    ("EVENT_V1", EVENT_V1_FEATURE),
    ("COPY", COPY_FEATURE),
    ("STRIDED_COPY", STRIDED_COPY_FEATURE),
    ("INDIRECT_BROADCAST", INDIRECT_BROADCAST_FEATURE),
    ("VECTOR_ADD", VECTOR_ADD_FEATURE),
];

/// ABI-v1 descriptor opcode assignments.
pub const OPCODES: [OpcodeSpec; 4] = [
    OpcodeSpec {
        name: "COPY",
        value: COPY_OPCODE,
    },
    OpcodeSpec {
        name: "STRIDED_COPY",
        value: STRIDED_COPY_OPCODE,
    },
    OpcodeSpec {
        name: "BROADCAST_COPY",
        value: BROADCAST_COPY_OPCODE,
    },
    OpcodeSpec {
        name: "VECTOR_ADD",
        value: VECTOR_ADD_OPCODE,
    },
];

/// ABI-v1 event-kind assignments.
pub const EVENT_KINDS: [NamedValue; 11] = [
    NamedValue::new("UNMAPPED_IOVA", 1),
    NamedValue::new("RANGE_OVERFLOW", 2),
    NamedValue::new("OUT_OF_BOUNDS", 3),
    NamedValue::new("INVALID_VERSION", 4),
    NamedValue::new("INVALID_SIZE", 5),
    NamedValue::new("INVALID_OPCODE", 6),
    NamedValue::new("INVALID_FLAGS", 7),
    NamedValue::new("INVALID_RESERVED", 8),
    NamedValue::new("INVALID_INDIRECT_COUNT", 9),
    NamedValue::new("INVALID_INDIRECT_ALIGNMENT", 10),
    NamedValue::new("VALUE_OUT_OF_RANGE", 11),
];

/// ABI-v1 fault-stage assignments.
pub const EVENT_STAGES: [NamedValue; 4] = [
    NamedValue::new("DESCRIPTOR", 1),
    NamedValue::new("INDIRECT_LIST", 2),
    NamedValue::new("SOURCE", 3),
    NamedValue::new("DESTINATION", 4),
];

/// ABI-v1 fault-access assignments.
pub const EVENT_ACCESSES: [NamedValue; 3] = [
    NamedValue::new("NONE", 0),
    NamedValue::new("READ", 1),
    NamedValue::new("WRITE", 2),
];

const DESCRIPTOR_FIELDS: [FieldSpec; 11] = [
    FieldSpec::new("version", 0, 2),
    FieldSpec::new("opcode", 2, 2),
    FieldSpec::new("flags", 4, 2),
    FieldSpec::new("size", 6, 2),
    FieldSpec::new("command_id", 8, 8),
    FieldSpec::new("word0", 16, 8),
    FieldSpec::new("word1", 24, 8),
    FieldSpec::new("word2", 32, 8),
    FieldSpec::new("word3", 40, 8),
    FieldSpec::new("word4", 48, 8),
    FieldSpec::new("word5", 56, 8),
];

const COMPLETION_FIELDS: [FieldSpec; 7] = [
    FieldSpec::new("version", 0, 2),
    FieldSpec::new("size", 2, 2),
    FieldSpec::new("status", 4, 2),
    FieldSpec::new("flags", 6, 2),
    FieldSpec::new("origin_queue_id", 8, 8),
    FieldSpec::new("command_id", 16, 8),
    FieldSpec::new("reserved", 24, 8),
];

const EVENT_FIELDS: [FieldSpec; 13] = [
    FieldSpec::new("version", 0, 2),
    FieldSpec::new("size", 2, 2),
    FieldSpec::new("kind", 4, 2),
    FieldSpec::new("flags", 6, 2),
    FieldSpec::new("origin_queue_id", 8, 8),
    FieldSpec::new("command_id", 16, 8),
    FieldSpec::new("fault_locator", 24, 8),
    FieldSpec::new("detail0", 32, 8),
    FieldSpec::new("detail1", 40, 8),
    FieldSpec::new("stage", 48, 2),
    FieldSpec::new("access", 50, 2),
    FieldSpec::new("reserved", 52, 4),
    FieldSpec::new("index", 56, 8),
];

/// Canonical ABI-v1 descriptor record.
pub const DESCRIPTOR: RecordSpec = RecordSpec {
    size: DESCRIPTOR_SIZE,
    alignment: DESCRIPTOR_ALIGNMENT,
    fields: &DESCRIPTOR_FIELDS,
};

/// Canonical ABI-v1 completion record.
pub const COMPLETION: RecordSpec = RecordSpec {
    size: COMPLETION_SIZE,
    alignment: COMPLETION_ALIGNMENT,
    fields: &COMPLETION_FIELDS,
};

/// Canonical ABI-v1 event record.
pub const EVENT: RecordSpec = RecordSpec {
    size: EVENT_SIZE,
    alignment: EVENT_ALIGNMENT,
    fields: &EVENT_FIELDS,
};
