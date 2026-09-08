//! Generated MMIO C-header conformance.

use std::fmt::Display;

use pdma_model_contract::abi::{
    header::render_c_header,
    mmio::{MmioAccess, MmioReset, PdmaMmioMap},
};

const COMMANDS: [(&str, u32); 22] = [
    ("GLOBAL_COMMAND_DISCOVER", 1),
    ("GLOBAL_COMMAND_READ_LIMITS", 2),
    ("GLOBAL_COMMAND_READ_STATUS", 3),
    ("GLOBAL_COMMAND_RESET", 4),
    ("QUEUE_COMMAND_ACTIVATE", 1),
    ("QUEUE_COMMAND_READ_STATUS", 2),
    ("QUEUE_COMMAND_PUBLISH_SQ_PRODUCER", 3),
    ("QUEUE_COMMAND_CAPTURE_SQ_PRODUCER", 4),
    ("QUEUE_COMMAND_PUBLISH_SQ_CONSUMER", 5),
    ("QUEUE_COMMAND_PUBLISH_CQ_PRODUCER", 6),
    ("QUEUE_COMMAND_PUBLISH_CQ_CONSUMER", 7),
    ("QUEUE_COMMAND_PUBLISH_EQ_PRODUCER", 8),
    ("QUEUE_COMMAND_PUBLISH_EQ_CONSUMER", 9),
    ("IRQ_COMMAND_CONFIGURE_ROUTE", 1),
    ("IRQ_COMMAND_ENABLE", 2),
    ("IRQ_COMMAND_DISABLE", 3),
    ("IRQ_COMMAND_MASK", 4),
    ("IRQ_COMMAND_UNMASK", 5),
    ("IRQ_COMMAND_ACKNOWLEDGE", 6),
    ("IRQ_COMMAND_OBSERVE_SOURCE", 7),
    ("IRQ_COMMAND_SNAPSHOT_LINE", 8),
    ("IRQ_COMMAND_CLAIM_NOTIFICATION", 9),
];

const RESULTS: [(&str, u32); 32] = [
    ("NOT_EXECUTED", 0),
    ("OK", 1),
    ("EMPTY", 2),
    ("UNSUPPORTED_VERSION", 0x100),
    ("UNKNOWN_FEATURE_BITS", 0x101),
    ("STALE_EPOCH", 0x102),
    ("INVALID_QUEUE", 0x103),
    ("INVALID_IRQ_SOURCE", 0x104),
    ("IRQ_SOURCE_ACTIVE", 0x105),
    ("INVALID_RING_DEPTH", 0x106),
    ("RING_EXTENT_OVERFLOW", 0x107),
    ("MISALIGNED_RING", 0x108),
    ("RING_EXTENT_MISMATCH", 0x109),
    ("UNEXPECTED_RING_RECORD", 0x10a),
    ("QUEUE_DEPTH_MISMATCH", 0x10b),
    ("EVENT_DEPTH_MISMATCH", 0x10c),
    ("CURSOR_REGRESSION", 0x10d),
    ("CURSOR_UPDATE_REGRESSION", 0x10e),
    ("CURSOR_DISTANCE_EXCEEDED", 0x10f),
    ("CURSOR_INCREMENT_OVERFLOW", 0x110),
    ("DEVICE_EPOCH_OVERFLOW", 0x111),
    ("UNKNOWN_STATUS_VALUE", 0x112),
    ("DMA_ADDRESS_EXHAUSTED", 0x113),
    ("DMA_STORAGE_EXHAUSTED", 0x114),
    ("DMA_UNMAPPED", 0x115),
    ("DMA_CROSSES_REGION", 0x116),
    ("DMA_INVALID_REGION", 0x117),
    ("DMA_OVERLAP", 0x118),
    ("DMA_STALE_STAGE", 0x119),
    ("INVALID_TRANSACTION", 0x11a),
    ("UNKNOWN_COMMAND", 0x11b),
    ("UNKNOWN_ENCODING", 0x11c),
];

const IRQ_STATE_BITS: [(&str, u32); 7] = [
    ("PRESENT", 1),
    ("ENABLED", 2),
    ("MASKED", 4),
    ("CAUSE_PENDING", 8),
    ("DELIVERABLE", 0x10),
    ("LINE_ASSERTED", 0x20),
    ("RECORDS_PENDING", 0x40),
];

const REGISTER_SENTINELS: [&str; 18] = [
    "#define PDMA_MODEL_MMIO_GLOBAL_MAGIC_OFFSET UINT32_C(0)",
    "#define PDMA_MODEL_MMIO_GLOBAL_MAGIC_WIDTH UINT32_C(4)",
    "#define PDMA_MODEL_MMIO_GLOBAL_MAGIC_ACCESS UINT32_C(0)",
    "#define PDMA_MODEL_MMIO_GLOBAL_MAGIC_RESET_CLASS UINT32_C(0)",
    "#define PDMA_MODEL_MMIO_GLOBAL_MAGIC_RESET_VALUE UINT64_C(1095582800)",
    "#define PDMA_MODEL_MMIO_GLOBAL_MAGIC_WRITABLE_MASK UINT64_C(0)",
    "#define PDMA_MODEL_MMIO_GLOBAL_REQ_FEATURES_OFFSET UINT32_C(16)",
    "#define PDMA_MODEL_MMIO_GLOBAL_REQ_FEATURES_WIDTH UINT32_C(8)",
    "#define PDMA_MODEL_MMIO_GLOBAL_REQ_FEATURES_ACCESS UINT32_C(1)",
    "#define PDMA_MODEL_MMIO_GLOBAL_REQ_FEATURES_RESET_CLASS UINT32_C(1)",
    "#define PDMA_MODEL_MMIO_GLOBAL_REQ_FEATURES_RESET_VALUE UINT64_C(0)",
    "#define PDMA_MODEL_MMIO_GLOBAL_REQ_FEATURES_WRITABLE_MASK UINT64_C(18446744073709551615)",
    "#define PDMA_MODEL_MMIO_GLOBAL_COMMAND_ACCESS UINT32_C(2)",
    "#define PDMA_MODEL_MMIO_GLOBAL_RESPONSE_EPOCH_RESET_CLASS UINT32_C(2)",
    "#define PDMA_MODEL_MMIO_GLOBAL_RESPONSE_EPOCH_RESET_VALUE UINT64_C(0)",
    "#define PDMA_MODEL_MMIO_QUEUE_EXPECTED_EPOCH_OFFSET UINT32_C(256)",
    "#define PDMA_MODEL_MMIO_IRQ_EXPECTED_EPOCH_OFFSET UINT32_C(512)",
    "#define PDMA_MODEL_MMIO_IRQ_RESPONSE_PENDING_MESSAGES_WRITABLE_MASK UINT64_C(0)",
];

#[test]
fn generated_header_matches_every_literal_mmio_macro() {
    // Given: independent literal register, command, result, and state-bit expectations.
    let header = render_c_header().expect("canonical schemas must resolve");

    // When: the complete generated MMIO section is inspected.
    let mmio_defines = header
        .lines()
        .filter(|line| line.starts_with("#define PDMA_MODEL_MMIO_"))
        .count();

    // Then: every approved macro is exact and no additional MMIO macro drifts in.
    assert_eq!(mmio_defines, 463);
    let symbols: std::collections::BTreeSet<_> = header
        .lines()
        .filter_map(|line| line.strip_prefix("#define PDMA_MODEL_MMIO_"))
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(symbols.len(), mmio_defines);
    assert_u32(&header, "ACCESS_RO", 0);
    assert_u32(&header, "ACCESS_RW", 1);
    assert_u32(&header, "ACCESS_WO", 2);
    assert_u32(&header, "RESET_CONST", 0);
    assert_u32(&header, "RESET_R0", 1);
    assert_u32(&header, "RESET_POST", 2);
    for expected in REGISTER_SENTINELS {
        assert!(header.lines().any(|actual| actual == expected));
    }

    let resolved = PdmaMmioMap::resolve().expect("canonical MMIO map must resolve");
    for register in resolved.registers() {
        let name = register
            .path()
            .strip_prefix("PdmaMmioMap.")
            .unwrap()
            .replace('.', "_")
            .to_ascii_uppercase();
        let definition = register.definition();
        assert_define(
            &header,
            &format!("{name}_OFFSET"),
            "UINT32_C",
            register.offset(),
        );
        assert_define(
            &header,
            &format!("{name}_WIDTH"),
            "UINT32_C",
            definition.width(),
        );
        assert_u32(
            &header,
            &format!("{name}_ACCESS"),
            access_code(definition.access()),
        );
        assert_u32(
            &header,
            &format!("{name}_RESET_CLASS"),
            reset_code(definition.reset_kind()),
        );
        assert_u64(
            &header,
            &format!("{name}_RESET_VALUE"),
            definition.reset_value(),
        );
        assert_u64(
            &header,
            &format!("{name}_WRITABLE_MASK"),
            definition.writable_mask(),
        );
    }
    for (name, value) in COMMANDS {
        assert_u32(&header, name, value);
    }
    for (name, value) in RESULTS {
        assert_u32(&header, &format!("RESULT_{name}"), value);
    }
    for (name, value) in IRQ_STATE_BITS {
        assert_u32(&header, &format!("IRQ_STATE_{name}"), value);
    }
}

const fn access_code(access: MmioAccess) -> u32 {
    match access {
        MmioAccess::ReadOnly => 0,
        MmioAccess::ReadWrite => 1,
        MmioAccess::WriteOnly => 2,
    }
}

const fn reset_code(reset: MmioReset) -> u32 {
    match reset {
        MmioReset::Constant => 0,
        MmioReset::Zero => 1,
        MmioReset::PostReset => 2,
    }
}

fn assert_u32(header: &str, name: &str, value: u32) {
    assert_define(header, name, "UINT32_C", value);
}

fn assert_u64(header: &str, name: &str, value: u64) {
    assert_define(header, name, "UINT64_C", value);
}

fn assert_define(header: &str, name: &str, wrapper: &str, value: impl Display) {
    let expected = format!("#define PDMA_MODEL_MMIO_{name} {wrapper}({value})");
    assert!(
        header.lines().any(|actual| actual == expected),
        "missing {expected}"
    );
}
