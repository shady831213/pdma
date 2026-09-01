#[test]
fn device_has_one_contract_decoder_and_no_competing_codec_or_table() {
    // Given: the complete production MMIO dispatch and register-effect sources.
    let root = include_str!("../../runtime/mmio.rs");
    let sources = [
        include_str!("../../register_file/bank.rs"),
        include_str!("dispatch.rs"),
        include_str!("global.rs"),
        include_str!("queue.rs"),
        include_str!("irq.rs"),
        include_str!("reset.rs"),
    ];

    // When: decoder calls and duplicate boundary mechanisms are counted.
    let read_decoders = root.matches("decode_read(offset, width)?").count();
    let write_decoders = root.matches("decode_write(offset, bytes)?").count();

    // Then: each access has one contract decoder and no device-owned competing codec.
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
