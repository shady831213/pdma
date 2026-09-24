#![doc = "Context table publication and maintenance boundaries."]
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::hw_components::context::{
    ContextEntryRegisters, ContextManagementRegisters, ContextTableRegisters,
};
use pdma_model_dma::Dma;
use pdma_model_hw_components::context::ContextTable;
use pdma_model_hw_components::windows::ChannelWindows;

#[test]
fn drain_preserves_valid_until_firmware_explicitly_releases_the_entry() {
    // Given: a published Default binding and an idle DMA with no bound ring.
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let mut dma = Dma::new(SharedMemory::new(), 1).unwrap();
    let mut table = ContextTable::new(2, 1, 0).unwrap();
    let entry = ContextTableRegisters::for_channels(2, 1)
        .unwrap()
        .entries()
        .offset(0)
        .unwrap();
    table
        .management_write(
            (
                entry + ContextEntryRegisters::VALID.offset(),
                &1_u32.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    assert!(
        table
            .management_write(
                (
                    entry + ContextEntryRegisters::OWNER.offset(),
                    &1_u32.to_le_bytes()
                ),
                (&mut dma, &windows)
            )
            .is_err()
    );
    assert!(
        table
            .management_write(
                (
                    entry + ContextEntryRegisters::ATTRIBUTES.offset(),
                    &1_u32.to_le_bytes()
                ),
                (&mut dma, &windows),
            )
            .is_err()
    );
    // When: hardware completes Drain through the actual DMA maintenance engine.
    table
        .management_write(
            (
                ContextManagementRegisters::REQUEST.offset(),
                &(1_u64 << 32).to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    table.step(&dma);
    // Then: Drain retains validity; firmware releases the slot before reconfiguring it.
    let mut result = [0; 4];
    table
        .management_read(ContextManagementRegisters::RESULT.offset(), &mut result)
        .unwrap();
    assert_eq!(u32::from_le_bytes(result), 2);
    table
        .management_read(entry + ContextEntryRegisters::VALID.offset(), &mut result)
        .unwrap();
    assert_eq!(u32::from_le_bytes(result), 1);
    table
        .management_write(
            (
                entry + ContextEntryRegisters::VALID.offset(),
                &0_u32.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    table
        .management_write(
            (
                entry + ContextEntryRegisters::OWNER.offset(),
                &1_u32.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    table
        .management_write(
            (
                entry + ContextEntryRegisters::VALID.offset(),
                &1_u32.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
}

#[test]
fn failed_drain_retains_live_binding_and_reports_failed_channel() {
    // Given: a live binding owned by a Function with a faulted DMA channel.
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let mut dma = Dma::new(SharedMemory::new(), 1).unwrap();
    let mut table = ContextTable::new(1, 1, 0).unwrap();
    let entry = ContextTableRegisters::for_channels(1, 1)
        .unwrap()
        .entries()
        .offset(0)
        .unwrap();
    table
        .management_write(
            (
                entry + ContextEntryRegisters::VALID.offset(),
                &1_u32.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    dma.signal_fatal();
    // When: firmware requests Drain and hardware observes the fault.
    table
        .management_write(
            (
                ContextManagementRegisters::REQUEST.offset(),
                &(1_u64 << 32).to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    table.step(&dma);
    // Then: failure names channel zero, retains validity and keeps live attributes immutable.
    let mut result = [0; 4];
    table
        .management_read(ContextManagementRegisters::RESULT.offset(), &mut result)
        .unwrap();
    assert_eq!(u32::from_le_bytes(result), 5);
    table
        .management_read(
            ContextTableRegisters::for_channels(1, 1)
                .unwrap()
                .failed()
                .offset(0)
                .unwrap(),
            &mut result,
        )
        .unwrap();
    assert_eq!(u32::from_le_bytes(result), 1);
    assert!(
        table
            .management_write(
                (
                    entry + ContextEntryRegisters::OWNER.offset(),
                    &1_u32.to_le_bytes()
                ),
                (&mut dma, &windows)
            )
            .is_err()
    );
    table
        .management_read(entry + ContextEntryRegisters::VALID.offset(), &mut result)
        .unwrap();
    assert_eq!(u32::from_le_bytes(result), 1);
}

#[test]
fn invalid_attributes_cannot_publish_and_host_cannot_access_the_table() {
    // Given: a staged entry with an overflowing PASID.
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let mut dma = Dma::new(SharedMemory::new(), 1).unwrap();
    let mut table = ContextTable::new(1, 1, 0).unwrap();
    let entry = ContextTableRegisters::for_channels(1, 1)
        .unwrap()
        .entries()
        .offset(0)
        .unwrap();
    let attrs = entry + ContextEntryRegisters::ATTRIBUTES.offset();
    table
        .management_write((attrs, &1_u32.to_le_bytes()), (&mut dma, &windows))
        .unwrap();
    table
        .management_write(
            (
                attrs
                    + pdma_model_contract::hw_components::context::ContextAttributeRegisters::PASID
                        .offset(),
                &(1_u32 << 20).to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    // When: firmware attempts publication and Host attempts management access.
    let published = table.management_write(
        (
            entry + ContextEntryRegisters::VALID.offset(),
            &1_u32.to_le_bytes(),
        ),
        (&mut dma, &windows),
    );
    // Then: neither invalid access attributes nor Host management accesses are accepted.
    assert!(published.is_err());
    assert!(table.host_read(entry, &mut [0; 4]).is_err());
    assert!(table.host_write(entry, &1_u32.to_le_bytes()).is_err());
}
