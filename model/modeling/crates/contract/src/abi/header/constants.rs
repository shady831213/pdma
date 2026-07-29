use std::fmt::Write as _;

use super::super::{
    ABI_VERSION, COMPLETION_ALIGNMENT, COMPLETION_FLAGS_MASK, COMPLETION_SIZE,
    DESCRIPTOR_ALIGNMENT, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE, EVENT_ACCESSES, EVENT_ALIGNMENT,
    EVENT_FLAGS_MASK, EVENT_KINDS, EVENT_SIZE, EVENT_STAGES, FEATURES,
    INDIRECT_DESTINATION_ENTRY_SIZE, INDIRECT_ENTRY_ALIGNMENT, KNOWN_FEATURE_MASK,
    MAX_INDIRECT_DESTINATIONS, MAX_RING_DEPTH, MAX_STRIDE, MAX_STRIDED_ROWS, MAX_TRANSFER_BYTES,
    OPCODES, SUCCESS_STATUS,
};

const U16_VALUES: [(&str, u16); 9] = [
    ("PDMA_MODEL_ABI_VERSION", ABI_VERSION),
    ("PDMA_MODEL_ABI_DESCRIPTOR_SIZE", DESCRIPTOR_SIZE),
    ("PDMA_MODEL_ABI_COMPLETION_SIZE", COMPLETION_SIZE),
    ("PDMA_MODEL_ABI_EVENT_SIZE", EVENT_SIZE),
    (
        "PDMA_MODEL_ABI_INDIRECT_DESTINATION_ENTRY_SIZE",
        INDIRECT_DESTINATION_ENTRY_SIZE,
    ),
    (
        "PDMA_MODEL_ABI_DESCRIPTOR_FLAGS_MASK",
        DESCRIPTOR_FLAGS_MASK,
    ),
    (
        "PDMA_MODEL_ABI_COMPLETION_FLAGS_MASK",
        COMPLETION_FLAGS_MASK,
    ),
    ("PDMA_MODEL_ABI_EVENT_FLAGS_MASK", EVENT_FLAGS_MASK),
    ("PDMA_MODEL_ABI_STATUS_SUCCESS", SUCCESS_STATUS),
];

const U32_VALUES: [(&str, u32); 5] = [
    ("PDMA_MODEL_ABI_MAX_RING_DEPTH", MAX_RING_DEPTH),
    (
        "PDMA_MODEL_ABI_MAX_INDIRECT_DESTINATIONS",
        MAX_INDIRECT_DESTINATIONS,
    ),
    ("PDMA_MODEL_ABI_MAX_TRANSFER_BYTES", MAX_TRANSFER_BYTES),
    ("PDMA_MODEL_ABI_MAX_STRIDED_ROWS", MAX_STRIDED_ROWS),
    ("PDMA_MODEL_ABI_MAX_STRIDE", MAX_STRIDE),
];

const U64_VALUES: [(&str, u64); 5] = [
    ("PDMA_MODEL_ABI_DESCRIPTOR_ALIGNMENT", DESCRIPTOR_ALIGNMENT),
    ("PDMA_MODEL_ABI_COMPLETION_ALIGNMENT", COMPLETION_ALIGNMENT),
    ("PDMA_MODEL_ABI_EVENT_ALIGNMENT", EVENT_ALIGNMENT),
    (
        "PDMA_MODEL_ABI_INDIRECT_ENTRY_ALIGNMENT",
        INDIRECT_ENTRY_ALIGNMENT,
    ),
    ("PDMA_MODEL_ABI_KNOWN_FEATURE_MASK", KNOWN_FEATURE_MASK),
];

pub(super) fn push(output: &mut String) {
    for &(name, value) in &U16_VALUES {
        push_u16(output, name, value);
    }
    for &(name, value) in &U32_VALUES {
        push_u32(output, name, value);
    }
    for &(name, value) in &U64_VALUES {
        push_u64(output, name, value);
    }
    for &(name, value) in &FEATURES {
        push_u64(output, &format!("PDMA_MODEL_ABI_FEATURE_{name}"), value);
    }
    for opcode in OPCODES {
        push_u16(
            output,
            &format!("PDMA_MODEL_ABI_OPCODE_{}", opcode.name()),
            opcode.value(),
        );
    }
    push_named_values(output, "PDMA_MODEL_ABI_EVENT_KIND", &EVENT_KINDS);
    push_named_values(output, "PDMA_MODEL_ABI_EVENT_STAGE", &EVENT_STAGES);
    push_named_values(output, "PDMA_MODEL_ABI_EVENT_ACCESS", &EVENT_ACCESSES);
}

fn push_u16(output: &mut String, name: &str, value: u16) {
    let _ = writeln!(output, "#define {name} UINT16_C({value})");
}

fn push_u32(output: &mut String, name: &str, value: u32) {
    let _ = writeln!(output, "#define {name} UINT32_C({value})");
}

fn push_u64(output: &mut String, name: &str, value: u64) {
    let _ = writeln!(output, "#define {name} UINT64_C({value})");
}

fn push_named_values(output: &mut String, prefix: &str, values: &[super::super::NamedValue]) {
    for value in values {
        push_u16(output, &format!("{prefix}_{}", value.name), value.value);
    }
}
