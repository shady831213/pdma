use super::*;
use crate::harness::page_fault::platform::Sources;
use hardware_interface_runtime::{SharedMemory, SharedMemoryAllocator};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::{ContextAttributes, FunctionId};

#[test]
fn atomic_fault_preserves_bits_and_opaque_origin_before_repair() {
    use hardware_interface_runtime::{
        AtomicAccessError, AtomicOperation, AtomicOrder, AtomicUpdate,
    };
    // Given: an initialized atomic word on an evicted page with a platform source tag.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let address = allocator.allocate(8, MemoryAlignment::ONE).unwrap();
    let (reports, mut receiver) = mpsc::channel(1);
    let sources = Sources::new(1);
    let request = TaggedRequest {
        identity: MemoryIdentity {
            function: FunctionId::new(1),
            attributes: ContextAttributes::DEFAULT,
        },
        ch_tag: sources.ch_tag(ChannelId::new(0)),
    };
    let memory = Paged {
        memory: allocator.memory_access(),
        pages: PageTable::default(),
        reports,
    };
    memory
        .memory
        .write(address, MemoryAlignment::ONE, &12_u64.to_le_bytes())
        .unwrap();
    memory.pages.evict(address);
    // When: a read-modify-write encounters the missing mapping.
    let result = memory.atomic64_with_sideband(
        &request,
        address,
        AtomicOperation::Update {
            update: AtomicUpdate::Add(5),
            order: AtomicOrder::AcquireRelease,
        },
    );
    // Then: the original source is reported and no part of the word was changed.
    assert_eq!(
        result,
        Err(AtomicAccessError::Access(PageError::PageFault { address }))
    );
    assert_eq!(
        receiver.try_recv().unwrap(),
        FaultReport {
            identity: request.identity,
            ch_tag: request.ch_tag.unwrap(),
            address,
            access: FaultAccess::Write,
        }
    );
    assert_eq!(
        memory
            .memory
            .read_typed::<[u8; 8]>(address, MemoryAlignment::ONE)
            .unwrap(),
        12_u64.to_le_bytes()
    );
}

#[test]
fn same_space_channel_tags_preserve_origins_for_duplicate_reports() {
    // Given: two channel requests touch the same missing page in the same address space.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let address = allocator.allocate(4, MemoryAlignment::ONE).unwrap();
    let (reports, mut receiver) = mpsc::channel(3);
    let sources = Sources::new(2);
    let identity = MemoryIdentity {
        function: FunctionId::new(1),
        attributes: ContextAttributes::DEFAULT,
    };
    let memory = Paged {
        memory: allocator.memory_access(),
        pages: PageTable::default(),
        reports,
    };
    memory.pages.evict(address);
    // When: both accesses fault before either owner repairs the shared page.
    for channel in [0, 1, 0] {
        assert_eq!(
            memory.write_with_sideband(
                &TaggedRequest {
                    identity,
                    ch_tag: sources.ch_tag(ChannelId::new(channel))
                },
                address,
                MemoryAlignment::ONE,
                &[1; 4]
            ),
            Err(PageError::PageFault { address })
        );
    }
    // Then: platform reports must retain enough origin information for separate resumes.
    let first = receiver.try_recv().unwrap();
    let second = receiver.try_recv().unwrap();
    let duplicate = receiver.try_recv().unwrap();
    assert_ne!(
        first, second,
        "same-space reports lost the originating channel"
    );
    assert_eq!(duplicate, first);
}

#[test]
fn platform_write_fault_reports_identity_without_committing_bytes() {
    use hardware_interface_runtime::{SharedMemory, SharedMemoryAllocator};
    use pdma_model_contract::memory::{ContextAttributes, FunctionId};
    // Given: one missing page backed by initialized storage and an independent fault receiver.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let address = allocator.allocate(4, MemoryAlignment::ONE).unwrap();
    let (reports, mut receiver) = mpsc::channel(1);
    let sources = Sources::new(1);
    let request = TaggedRequest {
        identity: MemoryIdentity {
            function: FunctionId::new(1),
            attributes: ContextAttributes::DEFAULT,
        },
        ch_tag: sources.ch_tag(ChannelId::new(0)),
    };
    let memory = Paged {
        memory: allocator.memory_access(),
        pages: PageTable::default(),
        reports,
    };
    memory
        .memory
        .write(address, MemoryAlignment::ONE, &[1, 2, 3, 4])
        .unwrap();
    memory.pages.evict(address);
    // When: a payload write accesses the missing page.
    let result = memory.write_with_sideband(&request, address, MemoryAlignment::ONE, &[5, 6, 7, 8]);
    // Then: the platform identifies the fault and the entire failed write stays uncommitted.
    assert_eq!(result, Err(PageError::PageFault { address }));
    assert_eq!(
        receiver.try_recv().unwrap(),
        FaultReport {
            identity: request.identity,
            ch_tag: request.ch_tag.unwrap(),
            address,
            access: FaultAccess::Write
        }
    );
    let mut bytes = [0; 4];
    memory
        .memory
        .read(address, MemoryAlignment::ONE, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [1, 2, 3, 4]);
}
