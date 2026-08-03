#[test]
fn device_source_has_one_contract_decoder_and_no_competing_codec_or_table() {
    // Given: the complete device-owned MMIO source framework.
    let root = include_str!("../../src/runner/mmio.rs");
    let sources = [
        include_str!("../../src/runner/mmio/bank.rs"),
        include_str!("../../src/runner/mmio/dispatch.rs"),
        include_str!("../../src/runner/mmio/global.rs"),
        include_str!("../../src/runner/mmio/queue.rs"),
        include_str!("../../src/runner/mmio/irq.rs"),
        include_str!("../../src/runner/mmio/reset.rs"),
    ];

    // When: decoder calls and forbidden duplicate boundary mechanisms are counted.
    let read_decoders = root.matches("decode_read(offset, width)?").count();
    let write_decoders = root.matches("decode_write(offset, bytes)?").count();

    // Then: each access has one decoder and device source owns no competing mechanism.
    assert_eq!(read_decoders, 1);
    assert_eq!(write_decoders, 1);
    for source in sources {
        assert!(!source.contains("MMIO_REGISTERS"));
        assert!(!source.contains("RegisterSpec"));
        assert!(!source.contains("enum MmioAccessError"));
        assert!(!source.contains("from_le_bytes"));
        assert!(!source.contains(".to_le_bytes"));
    }
}
