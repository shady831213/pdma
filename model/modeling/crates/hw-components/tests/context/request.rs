//! Atomic selector/action submission at the hardware access boundary.
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, ResponseRegisters,
};
use pdma_model_contract::hw_components::context::{
    ContextEntryRegisters, ContextManagementRegisters, ContextTableRegisters,
};
use pdma_model_dma::Dma;
use pdma_model_hw_components::context::ContextTable;
use pdma_model_hw_components::windows::ChannelWindows;

fn read(table: &mut ContextTable, offset: usize) -> u32 {
    let mut bytes = [0; 4];
    assert_eq!(table.management_read(offset, &mut bytes), Ok(()));
    u32::from_le_bytes(bytes)
}

#[test]
fn request_rejects_torn_or_invalid_writes_before_any_effect() {
    // Given: only context 1 is valid; context 0 must not be selected accidentally.
    let mut dma = Dma::new((), 1).unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let mut table = ContextTable::new(2, 1, 0).unwrap();
    let valid = ContextTableRegisters::for_channels(2, 1)
        .unwrap()
        .entries()
        .offset(1)
        .unwrap()
        + ContextEntryRegisters::VALID.offset();
    table
        .management_write((valid, &1_u32.to_le_bytes()), (&mut dma, &windows))
        .unwrap();
    let request = ContextManagementRegisters::REQUEST.offset();
    // When: writes split selector/action, use an invalid action, or set reserved bits.
    for (offset, bytes) in [
        (request, 1_u32.to_le_bytes().to_vec()),
        (request + 4, 1_u32.to_le_bytes().to_vec()),
        (request, ((3_u64 << 32) | 1).to_le_bytes().to_vec()),
        (
            request,
            ((1_u64 << 63) | (1_u64 << 32) | 1).to_le_bytes().to_vec(),
        ),
    ] {
        assert!(
            table
                .management_write((offset, &bytes), (&mut dma, &windows))
                .is_err()
        );
        // Then: neither the selector nor the completion state changes.
        assert_eq!(
            read(&mut table, ContextManagementRegisters::RESULT.offset()),
            0
        );
        let mut raw = [0; 8];
        table.management_read(request, &mut raw).unwrap();
        assert_eq!(raw, [0; 8]);
    }
    table
        .management_write(
            (request, &((1_u64 << 32) | 1).to_le_bytes()),
            (&mut dma, &windows),
        )
        .unwrap();
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        2
    );
    assert_eq!(read(&mut table, valid), 1);
}

#[test]
fn pending_request_cannot_be_replaced_or_have_its_binding_invalidated() {
    // Given: a published command keeps context maintenance pending.
    let mut dma = Dma::new(hardware_interface_runtime::SharedMemory::new(), 1).unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let mut table = ContextTable::new(2, 1, 0).unwrap();
    let valid = ContextTableRegisters::for_channels(2, 1)
        .unwrap()
        .entries()
        .offset(1)
        .unwrap()
        + ContextEntryRegisters::VALID.offset();
    table
        .management_write((valid, &1_u32.to_le_bytes()), (&mut dma, &windows))
        .unwrap();
    let base = ChannelId::new(0).register_offset(1).unwrap();
    for offset in [
        ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
        ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
    ] {
        dma.write(base + offset, &0x1000_u64.to_le_bytes()).unwrap();
    }
    for offset in [
        ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
        ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
        ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
        ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
    ] {
        dma.write(base + offset, &1_u32.to_le_bytes()).unwrap();
    }
    let request = ContextManagementRegisters::REQUEST.offset();
    table
        .management_write(
            (request, &((1_u64 << 32) | 1).to_le_bytes()),
            (&mut dma, &windows),
        )
        .unwrap();
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        1
    );
    // When: firmware attempts to overwrite the pending request or release its entry.
    assert!(
        table
            .management_write(
                (request, &(2_u64 << 32).to_le_bytes()),
                (&mut dma, &windows)
            )
            .is_err()
    );
    assert!(
        table
            .management_write((valid, &0_u32.to_le_bytes()), (&mut dma, &windows))
            .is_err()
    );
    // Then: the original request remains pending and its binding remains valid.
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        1
    );
    assert_eq!(read(&mut table, valid), 1);
    dma.signal_fatal();
    let mut pipeline = dma.service();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..8 {
        let _ = operation.step(&mut pipeline, &mut dma);
    }
    table.step(&dma);
    assert_eq!(
        read(&mut table, ContextManagementRegisters::RESULT.offset()),
        5
    );
}
