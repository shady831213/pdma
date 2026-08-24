//! Canonical numeric registry for PDMA ABI version 1.

use super::descriptor::{DescriptorSchema, IndirectDestinationEntrySchema};
use super::record::{CompletionSchema, EventSchema};
use super::{AbiControlError, AbiDecodeError};

/// Canonical generated record metadata.
pub type RecordSpec = hardware_interface_schema::RecordSpec;
/// Canonical generated field metadata.
pub type FieldSpec = hardware_interface_schema::FieldSpec;

/// ABI version implemented by this registry.
pub const ABI_VERSION: u16 = 1;

/// Descriptor record width in bytes.
pub const DESCRIPTOR_SIZE: u16 = fixed_record_size(DescriptorSchema::SPEC.size());
/// Completion record width in bytes.
pub const COMPLETION_SIZE: u16 = fixed_record_size(CompletionSchema::SPEC.size());
/// Event record width in bytes.
pub const EVENT_SIZE: u16 = fixed_record_size(EventSchema::SPEC.size());
/// Indirect destination-list entry width in bytes.
pub const INDIRECT_DESTINATION_ENTRY_SIZE: u16 =
    fixed_record_size(IndirectDestinationEntrySchema::SPEC.size());

/// Required descriptor base alignment.
pub const DESCRIPTOR_ALIGNMENT: u64 = DescriptorSchema::SPEC.alignment() as u64;
/// Required completion-ring base alignment.
pub const COMPLETION_ALIGNMENT: u64 = CompletionSchema::SPEC.alignment() as u64;
/// Required event-ring base alignment.
pub const EVENT_ALIGNMENT: u64 = EventSchema::SPEC.alignment() as u64;
/// Required indirect destination-entry alignment.
pub const INDIRECT_ENTRY_ALIGNMENT: u64 = IndirectDestinationEntrySchema::SPEC.alignment() as u64;

const fn fixed_record_size(size: usize) -> u16 {
    assert!(size <= u16::MAX as usize);
    let bytes = size.to_le_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

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

/// Validated ABI-v1 discovery features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiV1Features(u64);

impl AbiV1Features {
    /// Returns every discovery feature implemented by ABI v1.
    pub const fn all() -> Self {
        Self(KNOWN_FEATURE_MASK)
    }

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

/// Canonical ABI-v1 descriptor record.
pub const DESCRIPTOR: RecordSpec = DescriptorSchema::SPEC;

/// Canonical ABI-v1 completion record.
pub const COMPLETION: RecordSpec = CompletionSchema::SPEC;

/// Canonical ABI-v1 event record.
pub const EVENT: RecordSpec = EventSchema::SPEC;

/// Canonical ABI-v1 indirect destination-list entry.
pub const INDIRECT_DESTINATION_ENTRY: RecordSpec = IndirectDestinationEntrySchema::SPEC;
