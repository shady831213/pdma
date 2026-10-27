use crate::harness::runtime::memory::{FunctionSpaces, RouteError, RoutedMemory};
use hardware_interface_memory::{
    AtomicAccessError, AtomicOperation, AtomicWriteOrder, MemoryAccess, MemoryAddress,
    MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::{
    dma::{ChannelId, ContextId, DmaAccess},
    hw_components::{
        context::{ContextAttributeRegisters, ContextEntryRegisters, ContextTableRegisters},
        windows::WindowControl,
    },
    memory::{ContextAttributes, ContextAttributesAddressSpace, FunctionId, MemoryIdentity},
};
use pdma_model_dma::Dma;
use pdma_model_hw_components::{
    context::{ContextTable, MemoryError},
    windows::ChannelWindows,
};
use std::sync::{Arc, Mutex};

const ADDRESS: MemoryAddress = MemoryAddress::new(0x1000);
const BASE: usize = 0x10000;

mod errors;
mod tags;

fn space(value: u64) -> SharedMemory {
    let memory = SharedMemory::new();
    memory
        .add_regions(&[MemoryRegion::try_new(ADDRESS, 8, MemoryAlignment::ONE).unwrap()])
        .unwrap();
    memory
        .write(ADDRESS, MemoryAlignment::ONE, &value.to_le_bytes())
        .unwrap();
    memory
}

fn access(context: u32) -> DmaAccess {
    DmaAccess {
        channel: ChannelId::new(0),
        context: ContextId::new(context),
    }
}

#[test]
fn atomic_words_use_retained_owner_when_window_is_closed() {
    // Given: identical addresses in two Functions and a closed work window owned by Function one.
    let first = space(11);
    let second = space(22);
    let mut windows = ChannelWindows::new(1, 0, u32::try_from(BASE).unwrap()).unwrap();
    windows
        .management_write(BASE + WindowControl::OWNER.offset(), &1_u32.to_le_bytes())
        .unwrap();
    let memory = RoutedMemory::new(
        Arc::new(Mutex::new(windows)),
        vec![first.clone(), second.clone()],
    )
    .unwrap();
    // When: both atomic widths pass through the managed channel path.
    let old32 = memory.atomic32_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Store {
            value: 33,
            order: AtomicWriteOrder::Release,
        },
    );
    let old64 = memory.atomic64_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Store {
            value: 44,
            order: AtomicWriteOrder::Release,
        },
    );
    // Then: both return old bits and only the owner backing changes.
    assert_eq!(old32, Ok(22));
    assert_eq!(old64, Ok(33));
    assert_eq!(
        first
            .read_typed::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        11_u64.to_le_bytes()
    );
    assert_eq!(
        second
            .read_typed::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        44_u64.to_le_bytes()
    );
}

#[test]
fn atomic_contexts_isolate_pasid_zero_and_deny_foreign_owner_before_modification() {
    // Given: Default, PASID zero and foreign Function spaces sharing the same address.
    let windows = ChannelWindows::new(1, 0, u32::try_from(BASE).unwrap()).unwrap();
    let mut dma = Dma::new(SharedMemory::new(), 1).unwrap();
    let mut table = ContextTable::new(3, 1, 0).unwrap();
    let layout = ContextTableRegisters::for_channels(3, 1).unwrap();
    let memories = [space(11), space(22), space(33)];
    let mut backing = Vec::new();
    for (index, (function, kind)) in [(0_u32, 0_u32), (0, 1), (1, 0)].into_iter().enumerate() {
        let base = layout.entries().offset(index).unwrap();
        for (offset, value) in [
            (ContextEntryRegisters::OWNER.offset(), function),
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
        backing.push((
            MemoryIdentity {
                function: FunctionId::new(function),
                attributes: ContextAttributes {
                    address_space: if kind == 0 {
                        ContextAttributesAddressSpace::Default
                    } else {
                        ContextAttributesAddressSpace::Pasid
                    },
                    pasid: 0,
                },
            },
            memories.get(index).unwrap().clone(),
        ));
    }
    let memory = RoutedMemory::with_contexts(
        Arc::new(Mutex::new(windows)),
        Arc::new(Mutex::new(table)),
        FunctionSpaces {
            backing,
            bind: None,
        },
    );
    // When: PASID zero is updated and a foreign context attempts both atomic widths.
    let changed32 = memory.atomic32_with_sideband(
        &access(1),
        ADDRESS,
        AtomicOperation::Store {
            value: 55,
            order: AtomicWriteOrder::Relaxed,
        },
    );
    let changed = memory.atomic64_with_sideband(
        &access(1),
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Relaxed,
        },
    );
    let denied32 = memory.atomic32_with_sideband(
        &access(2),
        ADDRESS,
        AtomicOperation::Store {
            value: 88,
            order: AtomicWriteOrder::Relaxed,
        },
    );
    let denied64 = memory.atomic64_with_sideband(
        &access(2),
        ADDRESS,
        AtomicOperation::Store {
            value: 88,
            order: AtomicWriteOrder::Relaxed,
        },
    );
    // Then: the PASID space alone changes, and authorization is retained inside Access errors.
    assert_eq!(changed32, Ok(22));
    assert_eq!(changed, Ok(55));
    let rejection = || {
        AtomicAccessError::Access(
            RouteError::<hardware_interface_memory::MemoryAccessError>::Context(
                MemoryError::OwnerMismatch {
                    context: ContextId::new(2),
                    owner: FunctionId::new(0),
                },
            ),
        )
    };
    assert_eq!(denied32, Err(rejection()));
    assert_eq!(denied64, Err(rejection()));
    for (memory, value) in memories.iter().zip([11_u64, 99, 33]) {
        assert_eq!(
            memory
                .read_typed::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
                .unwrap(),
            value.to_le_bytes()
        );
    }
}
