use crate::support::{ADDRESS, Spaces};
use hardware_interface_io::memory::SharedMemory;
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelId, ContextId, DmaAccess};
use pdma_model_contract::hw_components::context::{
    ContextAttributeRegisters, ContextEntryRegisters, ContextTableRegisters,
};
use pdma_model_dma::Dma;
use pdma_model_hw_components::context::ContextTable;
use pdma_model_hw_components::windows::ChannelWindows;

#[test]
fn memory_routes_default_and_pasid_zero_and_denies_foreign_or_unallocated_entries() {
    // Given: identical addresses in Default, PASID zero, and another Function.
    let spaces = Spaces::new().unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let mut dma = Dma::new(SharedMemory::new(), 1).unwrap();
    let mut table = ContextTable::new(4, 2, 0).unwrap();
    let layout = ContextTableRegisters::for_channels(4, 2).unwrap();
    for (index, owner, kind) in [(0, 0_u32, 0_u32), (1, 0, 1), (2, 1, 0)] {
        let base = layout.entries().offset(index).unwrap();
        for (offset, value) in [
            (ContextEntryRegisters::OWNER.offset(), owner),
            (
                ContextEntryRegisters::ATTRIBUTES.offset()
                    + ContextAttributeRegisters::ADDRESS_SPACE.offset(),
                kind,
            ),
            (ContextEntryRegisters::VALID.offset(), 1),
        ] {
            table
                .management_write((base + offset, &value.to_le_bytes()), (&mut dma, &windows))
                .unwrap();
        }
    }
    let view = table.memory(&windows, &spaces);
    let access = |context| DmaAccess {
        channel: ChannelId::new(0),
        context: ContextId::new(context),
    };
    // When: each selector is used through the hardware memory view.
    let default = view
        .read_buffer_with_sideband::<[u8; 4]>(&access(0), ADDRESS, MemoryAlignment::ONE)
        .unwrap();
    let pasid = view
        .read_buffer_with_sideband::<[u8; 4]>(&access(1), ADDRESS, MemoryAlignment::ONE)
        .unwrap();
    view.write_with_sideband(&access(1), ADDRESS, MemoryAlignment::ONE, &[44; 4])
        .unwrap();
    // Then: independent bytes prove routing; rejected reads and writes preserve buffers and memory.
    assert_eq!(default, [11; 4]);
    assert_eq!(pasid, [22; 4]);
    for context in [2, 3, u32::MAX] {
        let mut bytes = [99; 4];
        assert!(
            view.read_with_sideband(&access(context), ADDRESS, MemoryAlignment::ONE, &mut bytes)
                .is_err()
        );
        assert!(
            view.write_with_sideband(&access(context), ADDRESS, MemoryAlignment::ONE, &[55; 4])
                .is_err()
        );
        assert!(
            view.validate_with_sideband(&access(context), ADDRESS, MemoryAlignment::ONE, 4)
                .is_err()
        );
        assert_eq!(bytes, [99; 4]);
    }
    for ((_, memory), byte) in spaces.0.iter().zip([11, 44, 33]) {
        assert_eq!(
            memory
                .read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
                .unwrap(),
            [byte; 4]
        );
    }
}
