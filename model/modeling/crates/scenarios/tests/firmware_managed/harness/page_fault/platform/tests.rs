use super::*;
use crate::harness::page_fault::{FaultAccess, FaultReport};
use pdma_model_contract::memory::ContextAttributes;

#[test]
fn bound_atomic_recovers_after_platform_repair_with_original_source() {
    use crate::harness::page_fault::memory::PageTable;
    use hardware_interface_runtime::{
        AtomicAccessError, AtomicOperation, AtomicOrder, AtomicUpdate, SharedMemory,
        SharedMemoryAllocator,
    };
    // Given: a platform-bound port and an initialized word in one evicted page.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let address = allocator.allocate(8, MemoryAlignment::ONE).unwrap();
    let memory = allocator.memory_access();
    memory
        .write(address, MemoryAlignment::ONE, &10_u64.to_le_bytes())
        .unwrap();
    let pages = PageTable::default();
    let (reports, mut receiver) = tokio::sync::mpsc::channel(1);
    let sources = Sources::new(1);
    let expected_tag = sources.ch_tag(ChannelId::new(0)).unwrap();
    let identity = MemoryIdentity {
        function: FunctionId::new(1),
        attributes: ContextAttributes::DEFAULT,
    };
    let backend = Backend::new(
        Paged {
            memory: memory.clone(),
            pages: pages.clone(),
            reports,
        },
        sources,
        identity,
    )
    .bind(MemoryRequest {
        channel: Some(ChannelId::new(0)),
        identity,
    });
    pages.evict(address);
    // When: a faulted atomic is retried after platform repair, then another width updates it.
    let operation = AtomicOperation::Update {
        update: AtomicUpdate::Add(5),
        order: AtomicOrder::AcquireRelease,
    };
    assert_eq!(
        backend.atomic64(address, operation),
        Err(AtomicAccessError::Access(PageError::PageFault { address }))
    );
    let report = receiver.try_recv().unwrap();
    assert!(pages.repair(address));
    let old64 = backend.atomic64(address, operation).unwrap();
    let old32 = backend
        .atomic32(
            address,
            AtomicOperation::Update {
                update: AtomicUpdate::Add(2),
                order: AtomicOrder::Relaxed,
            },
        )
        .unwrap();
    // Then: the fault retained source/identity, and each successful operation committed once.
    assert_eq!((report.ch_tag, report.identity), (expected_tag, identity));
    assert_eq!((old64, old32), (10, 15));
    assert_eq!(
        memory
            .read_typed::<[u8; 8]>(address, MemoryAlignment::ONE)
            .unwrap(),
        17_u64.to_le_bytes()
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn channel_tag_catalog_rejects_foreign_unknown_and_retired_sources() {
    // Given: one live grant in a two-channel platform instance.
    let sources = Sources::new(2);
    let channel = ChannelId::new(1);
    let function = FunctionId::new(3);
    let mut handler = sources.handler(function, &[channel]).unwrap();
    let report = FaultReport {
        identity: MemoryIdentity {
            function,
            attributes: ContextAttributes::DEFAULT,
        },
        ch_tag: sources.ch_tag(channel).unwrap(),
        address: 0x1000.into(),
        access: FaultAccess::Read,
    };
    let duplicate = report.clone();
    // When: report identity or source falls outside the grant, or its handler retires.
    let wrong_function = FaultReport {
        identity: MemoryIdentity {
            function: FunctionId::new(4),
            ..report.identity
        },
        ..report.clone()
    };
    let ungranted = FaultReport {
        ch_tag: sources.ch_tag(ChannelId::new(0)).unwrap(),
        ..report
    };
    let foreign = FaultReport {
        ch_tag: Sources::new(2).ch_tag(channel).unwrap(),
        ..report
    };
    // Then: only the current grant's source and Function resolve, with no partial invalid catalog.
    assert_eq!(handler.resolve(&report), Some(channel));
    assert_eq!(handler.resolve(&duplicate), Some(channel));
    assert_eq!(handler.resolve(&wrong_function), None);
    assert_eq!(handler.resolve(&ungranted), None);
    assert_eq!(handler.resolve(&foreign), None);
    assert!(
        sources
            .handler(function, &[channel, ChannelId::new(2)])
            .is_none()
    );
    handler.retire();
    assert_eq!(handler.resolve(&report), None);
    assert_eq!(handler.resolve(&duplicate), None);
}
