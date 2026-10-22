use super::{
    PlatformMemory,
    backend::{ADDRESS, Error},
    configured_table, sideband,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{
    dma::{ChannelId, ContextId, DmaAccess},
    memory::FunctionId,
};
use pdma_model_dma::MemoryFailure;
use pdma_model_hw_components::{context::MemoryError, windows::ChannelWindows};
use std::cell::Cell;

#[test]
fn atomic_context_authorization_precedes_unsupported_backend() {
    use hardware_interface_runtime::{AtomicAccessError, AtomicOperation, AtomicReadOrder};
    // Given: valid and foreign contexts over a backend without atomic support.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let table = configured_table(&windows).unwrap();
    let backend = PlatformMemory::new().unwrap();
    let port = table.memory(&windows, &backend).map_sideband(sideband);
    let valid = DmaAccess {
        channel: ChannelId::new(0),
        context: ContextId::new(0),
    };
    let foreign = DmaAccess {
        context: ContextId::new(1),
        ..valid
    };
    // When: valid and foreign requests ask for indivisible word loads.
    let valid32 = port.atomic32_with_sideband(
        &valid,
        ADDRESS,
        AtomicOperation::Load(AtomicReadOrder::Acquire),
    );
    let valid64 = port.atomic64_with_sideband(
        &valid,
        ADDRESS,
        AtomicOperation::Load(AtomicReadOrder::Acquire),
    );
    let denied = port.atomic64_with_sideband(
        &foreign,
        ADDRESS,
        AtomicOperation::Load(AtomicReadOrder::Acquire),
    );
    // Then: valid requests preserve Unsupported, while foreign requests fail authorization first.
    assert_eq!(valid32, Err(AtomicAccessError::Unsupported));
    assert_eq!(valid64, Err(AtomicAccessError::Unsupported));
    assert_eq!(
        denied,
        Err(AtomicAccessError::Access(MemoryError::OwnerMismatch {
            context: ContextId::new(1),
            owner: FunctionId::new(0)
        }))
    );
}

#[test]
fn rejected_context_accesses_never_call_platform_mapper_or_backend() {
    // Given: a mapper with observable effects behind context authorization.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let table = configured_table(&windows).unwrap();
    let backend = PlatformMemory::new().unwrap();
    let calls = Cell::new(0);
    let port = table
        .memory(&windows, &backend)
        .map_sideband(|access, identity| {
            calls.set(calls.get() + 1);
            sideband(access, identity)
        });
    // When: each memory method sees a foreign, invalid or unowned selector.
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
            2,
            0,
            MemoryError::UnownedChannel {
                channel: ChannelId::new(2),
            },
        ),
    ] {
        let access = DmaAccess {
            channel: ChannelId::new(channel),
            context: ContextId::new(context),
        };
        let mut bytes = [99; 4];
        for result in [
            port.validate_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, 4),
            port.read_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &mut bytes),
            port.write_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &[42; 4]),
        ] {
            assert_eq!(result, Err(expected.clone()));
        }
        assert_eq!(bytes, [99; 4]);
    }
    // Then: no platform callback or memory operation has occurred.
    assert_eq!(calls.get(), 0);
    assert!(backend.reports.borrow().is_empty());
    assert_eq!(
        backend
            .backing
            .read_typed::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [0; 4]
    );
}

#[test]
fn custom_sideband_preserves_typed_page_fault_errors_for_all_operations() {
    // Given: authorized traffic mapped to a platform backend reporting a missing page.
    let windows = ChannelWindows::new(2, 0, 0x10000).unwrap();
    let table = configured_table(&windows).unwrap();
    let backend = PlatformMemory::new().unwrap();
    backend.fault.set(true);
    let port = table.memory(&windows, &backend).map_sideband(sideband);
    let access = DmaAccess {
        channel: ChannelId::new(1),
        context: ContextId::new(0),
    };
    let mut bytes = [99; 4];
    // When: validation, read and write fail downstream of sideband mapping.
    let results = [
        port.validate_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, 4),
        port.read_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &mut bytes),
        port.write_with_sideband(&access, ADDRESS, MemoryAlignment::ONE, &[42; 4]),
    ];
    // Then: backend error identity and DMA classification survive without storage changes.
    for result in results {
        let error = result.unwrap_err();
        assert_eq!(error, MemoryError::Downstream(Error::MissingPage(ADDRESS)));
        assert_eq!(
            MemoryFailure::from(error),
            MemoryFailure::PageFault { address: ADDRESS }
        );
    }
    assert_eq!(bytes, [99; 4]);
    assert_eq!(
        backend
            .backing
            .read_typed::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        [0; 4]
    );
}
