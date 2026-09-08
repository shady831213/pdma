//! Direct register C-export conformance.

use pdma_model_contract::abi::header::render_c_header;

#[test]
fn header_exposes_indexed_hardware_blocks_without_command_protocol() {
    // Given: the canonical composed hardware register schema.
    let header = render_c_header().expect("canonical header");

    // When: a downstream consumer reads the generated address and permission constants.
    let defines: Vec<_> = header
        .lines()
        .filter(|line| line.starts_with("#define PDMA_MODEL_MMIO_"))
        .collect();

    // Then: queue indexing is explicit and hardware-owned cursors remain read-only.
    for expected in [
        "#define PDMA_MODEL_MMIO_QUEUE_BASE UINT32_C(64)",
        "#define PDMA_MODEL_MMIO_QUEUE_STRIDE UINT32_C(192)",
        "#define PDMA_MODEL_MMIO_IRQ_BASE UINT32_C(112)",
        "#define PDMA_MODEL_MMIO_IRQ_STRIDE UINT32_C(40)",
        "#define PDMA_MODEL_MMIO_QUEUE_SQ_CONSUMER_ACCESS UINT32_C(0)",
        "#define PDMA_MODEL_MMIO_QUEUE_SQ_PRODUCER_ACCESS UINT32_C(1)",
        "#define PDMA_MODEL_MMIO_IRQ_ACK_ACCESS UINT32_C(2)",
    ] {
        assert!(defines.contains(&expected), "missing {expected}");
    }
    assert!(!defines.iter().any(|line| line.contains("MMIO_RESULT_")));
    assert!(!defines.iter().any(|line| line.contains("_COMMAND_")));
    assert!(!defines.iter().any(|line| line.contains("_RESPONSE_")));
}
