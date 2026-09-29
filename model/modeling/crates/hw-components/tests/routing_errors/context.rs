use super::backend::{ADDRESS, BackendError, PagedMemory};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::{
    dma::{ChannelId, ContextId, DmaAccess},
    hw_components::context::{ContextEntryRegisters, ContextTableRegisters},
    memory::{ContextAttributes, FunctionId, MemoryIdentity},
};
use pdma_model_dma::{Dma, MemoryFailure};
use pdma_model_hw_components::{
    context::{ContextTable, MemoryError},
    windows::ChannelWindows,
};

fn configured_table(windows: &ChannelWindows) -> Result<ContextTable, Box<dyn std::error::Error>> {
    let mut dma = Dma::new(SharedMemory::new(), 1)?;
    let mut table = ContextTable::new(3, 1, 0)?;
    let layout = ContextTableRegisters::for_channels(3, 1)?;
    for (index, owner) in [(0, 0_u32), (1, 1)] {
        let base = layout
            .entries()
            .offset(index)
            .ok_or("missing context entry")?;
        for (offset, value) in [
            (ContextEntryRegisters::OWNER.offset(), owner),
            (ContextEntryRegisters::VALID.offset(), 1),
        ] {
            table.management_write((base + offset, &value.to_le_bytes()), (&mut dma, windows))?;
        }
    }
    Ok(table)
}

#[test]
fn authorized_context_retains_page_fault_address_without_mutating_storage() {
    // Given: an owned context resolving to a real backing with a missing mapping.
    let memory = PagedMemory::new(MemoryIdentity {
        function: FunctionId::new(0),
        attributes: ContextAttributes::DEFAULT,
    })
    .unwrap();
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let table = configured_table(&windows).unwrap();
    let port = table.memory(&windows, &memory);
    let access = DmaAccess {
        channel: ChannelId::new(0),
        context: ContextId::new(0),
    };
    let mut bytes = [99; 4];
    // When: validation, read and write traverse the authorized context.
    let failures = [
        port.validate_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, 4),
        port.read_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &mut bytes),
        port.write_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &[44; 4]),
    ];
    // Then: the typed source and DMA page-fault address survive; no failed write commits bytes.
    for result in failures {
        let error = result.unwrap_err();
        assert_eq!(
            error,
            MemoryError::Downstream(BackendError::MissingPage { address: ADDRESS })
        );
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            BackendError::MissingPage { address: ADDRESS }.to_string()
        );
        assert_eq!(
            MemoryFailure::from(error),
            MemoryFailure::PageFault { address: ADDRESS }
        );
    }
    assert_eq!(bytes, [99; 4]);
    assert_eq!(
        memory
            .backing
            .read_typed::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [13; 4]
    );
    memory.present.set(true);
    port.write_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &[44; 4])
        .unwrap();
    assert_eq!(
        port.read_typed_with_sideband::<[u8; 4]>(&access, ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [44; 4]
    );
}

#[test]
fn foreign_invalid_and_unowned_context_routes_fail_before_backend_access() {
    // Given: writable real storage; a forwarded unauthorized operation would change its bytes.
    let memory = PagedMemory::new(MemoryIdentity {
        function: FunctionId::new(0),
        attributes: ContextAttributes::DEFAULT,
    })
    .unwrap();
    memory.present.set(true);
    let windows = ChannelWindows::new(1, 0, 0x10000).unwrap();
    let table = configured_table(&windows).unwrap();
    let port = table.memory(&windows, &memory);
    // When: foreign, invalid, out-of-range and unowned selections request every access method.
    for (channel, context, expected) in [
        (
            0,
            1,
            MemoryError::OwnerMismatch {
                context: ContextId::new(1),
                owner: FunctionId::new(0),
            },
        ),
        (
            0,
            2,
            MemoryError::InvalidContext {
                context: ContextId::new(2),
            },
        ),
        (
            0,
            u32::MAX,
            MemoryError::InvalidContext {
                context: ContextId::new(u32::MAX),
            },
        ),
        (
            1,
            0,
            MemoryError::UnownedChannel {
                channel: ChannelId::new(1),
            },
        ),
    ] {
        let access = DmaAccess {
            channel: ChannelId::new(channel),
            context: ContextId::new(context),
        };
        let mut bytes = [99; 4];
        let failures = [
            port.validate_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, 4),
            port.read_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &mut bytes),
            port.write_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &[55; 4]),
        ];
        // Then: hardware rejection maps to Access and never enters the backing port.
        for result in failures {
            let error = result.unwrap_err();
            assert_eq!(error, expected);
            assert_eq!(MemoryFailure::from(error), MemoryFailure::Access);
        }
        assert_eq!(bytes, [99; 4]);
    }
    assert_eq!(memory.calls.get(), 0);
    assert_eq!(
        memory
            .backing
            .read_typed::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [13; 4]
    );
}
