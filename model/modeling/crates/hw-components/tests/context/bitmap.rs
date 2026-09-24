//! Retained multi-word maintenance failure snapshots through firmware MMIO.
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::context::{
    ContextEntryRegisters, ContextManagementRegisters, ContextTableRegisters,
};
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_dma::Dma;
use pdma_model_hw_components::context::ContextTable;
use pdma_model_hw_components::windows::ChannelWindows;

fn read(table: &mut ContextTable, offset: usize) -> u32 {
    let mut bytes = [0; 4];
    assert_eq!(table.management_read(offset, &mut bytes), Ok(()));
    u32::from_le_bytes(bytes)
}

#[test]
fn failure_collects_all_owned_channels_and_freezes_until_the_next_request() {
    // Given: Function zero owns channels 1, 33 and 64 of a 65-channel engine.
    let mut dma = Dma::new((), 65).unwrap();
    let mut windows = ChannelWindows::new(65, 0, 0x0010_0000).unwrap();
    for index in 0..65 {
        let owner = u32::from(![1, 33, 64].contains(&index));
        windows
            .management_write(
                0x0010_0000
                    + ChannelId::new(index).window_control_offset().unwrap()
                    + WindowControl::OWNER.offset(),
                &owner.to_le_bytes(),
            )
            .unwrap();
    }
    let mut table = ContextTable::new(2, 65, 0).unwrap();
    let layout = ContextTableRegisters::for_channels(2, 65).unwrap();
    let valid = layout.entries().offset(1).unwrap() + ContextEntryRegisters::VALID.offset();
    table
        .management_write((valid, &1_u32.to_le_bytes()), (&mut dma, &windows))
        .unwrap();
    dma.signal_fatal();
    // When: Drain encounters the engine fault across the Function's scope.
    table
        .management_write(
            (
                ContextManagementRegisters::REQUEST.offset(),
                &((1_u64 << 32) | 1).to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    // Then: Failed already has every owned bit, without foreign or unimplemented bits.
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        5
    );
    for (word, expected) in [2, 2, 1].into_iter().enumerate() {
        assert_eq!(
            read(&mut table, layout.failed().offset(word).unwrap()),
            expected
        );
    }
    assert_eq!(read(&mut table, valid), 1);
    assert!(
        table
            .management_write(
                (layout.failed().offset(0).unwrap(), &0_u32.to_le_bytes()),
                (&mut dma, &windows),
            )
            .is_err()
    );
    windows
        .management_write(
            0x0010_0000
                + ChannelId::new(2).window_control_offset().unwrap()
                + WindowControl::OWNER.offset(),
            &0_u32.to_le_bytes(),
        )
        .unwrap();
    table.step(&dma);
    table
        .management_write(
            (
                ContextManagementRegisters::REQUEST.offset(),
                &0_u64.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    assert_eq!(read(&mut table, layout.failed().offset(0).unwrap()), 2);
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        5
    );
    table
        .management_write(
            (
                ContextManagementRegisters::REQUEST.offset(),
                &((2_u64 << 32) | 1).to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    assert_eq!(read(&mut table, layout.failed().offset(0).unwrap()), 6);
    table
        .management_write(
            (
                ContextManagementRegisters::REQUEST.offset(),
                &((1_u64 << 32) | 9).to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        3
    );
    for word in 0..3 {
        assert_eq!(read(&mut table, layout.failed().offset(word).unwrap()), 0);
    }
}

#[test]
fn channel_growth_appends_bitmap_words_without_moving_entries() {
    // Given: identical context capacity with different instance channel counts.
    let one = ContextTableRegisters::for_channels(3, 32).unwrap();
    // When: more channels require additional bitmap words.
    for channels in [33, 64, 65] {
        let layout = ContextTableRegisters::for_channels(3, channels).unwrap();
        // Then: fixed registers, entries and the bitmap base retain their addresses.
        assert_eq!(layout.entries().base(), one.entries().base());
        assert_eq!(layout.entries().end(), one.entries().end());
        assert_eq!(layout.failed().base(), one.failed().base());
        assert!(layout.failed().base() >= layout.entries().end());
        assert_eq!(layout.failed().count(), channels.div_ceil(32));
        assert_eq!(layout.extent(), layout.failed().end());
    }
}
