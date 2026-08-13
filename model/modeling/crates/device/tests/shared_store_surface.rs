//! Consumer-level Task 16 shared-store ownership surface.

use std::sync::Arc;

use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, NoopRegisterObserver, SharedMemory,
};
use pdma_model_contract::ResourceConfig;
use pdma_model_device::compat::SyncDevice;
use pdma_model_device::{AsyncDevice, DeviceCore, PdmaMemoryError};

const ALIGNMENT: MemoryAlignment = match MemoryAlignment::try_new(1) {
    Ok(alignment) => alignment,
    Err(_) => panic!("one is a valid alignment"),
};

fn core(memory: Arc<SharedMemory>) -> DeviceCore {
    DeviceCore::new(
        memory,
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    )
}

#[tokio::test]
async fn task16_shared_store_surface() {
    // Given: separately owned stores injected into separately consumed device cores.
    let async_memory = Arc::new(SharedMemory::new());
    let sync_memory = Arc::new(SharedMemory::new());
    let zero_memory = Arc::new(SharedMemory::new());
    let (async_device, _worker) = AsyncDevice::from_core(core(Arc::clone(&async_memory)));
    let mut sync_device = SyncDevice::from_core(core(Arc::clone(&sync_memory)));
    let mut zero_device = SyncDevice::from_core(core(zero_memory));

    // When: each facade provisions and accesses its own real store.
    let async_region = async_device
        .handle()
        .allocate_memory_region(64)
        .await
        .expect("async allocation");
    async_device
        .handle()
        .write_memory(async_region.base(), &[0x11; 64])
        .await
        .expect("async write");
    async_device
        .handle()
        .allocate_memory_region(1)
        .await
        .expect("async identity allocation");
    let sync_region = sync_device
        .allocate_memory_region(64)
        .expect("sync allocation");
    sync_device
        .write_memory(sync_region.base(), &[0x22; 64])
        .expect("sync write");
    sync_device
        .allocate_memory_region(1)
        .expect("sync identity allocation");
    let zero = zero_device
        .allocate_memory_region(0)
        .expect("zero allocation");
    let one = zero_device
        .allocate_memory_region(1)
        .expect("one-byte allocation");

    // Then: identity, bytes, storage regions, and empty mapping rules are observable.
    assert!(!Arc::ptr_eq(&async_memory, &sync_memory));
    let async_bytes = async_device
        .handle()
        .read_memory(async_region.base(), 64)
        .await
        .expect("async bytes");
    let sync_bytes = sync_device
        .read_memory(sync_region.base(), 64)
        .expect("sync bytes");
    assert_eq!(async_bytes, vec![0x11; 64]);
    assert_eq!(sync_bytes, vec![0x22; 64]);
    assert_eq!(zero_device.read_memory(zero.base(), 0), Ok(Vec::new()));
    assert_eq!(
        zero_device.read_memory(zero.base(), 1),
        Err(PdmaMemoryError::CrossesRegion)
    );
    assert_eq!(
        zero_device.read_memory(MemoryAddress::new(zero.base().raw() + 1), 0),
        Err(PdmaMemoryError::Unmapped)
    );
    assert_eq!(async_memory.regions().len(), 2);
    assert_eq!(sync_memory.regions().len(), 2);
    println!(
        "async_bytes={};sync_bytes={};async_store_regions={};sync_store_regions={};zero_base={:#x};one_base={:#x};zero_empty=ok;zero_nonempty=crossing;zero_plus_one=unmapped;generation={}",
        async_bytes.len(),
        sync_bytes.len(),
        async_memory.regions().len(),
        sync_memory.regions().len(),
        zero.base().raw(),
        one.base().raw(),
        zero_device.memory_generation(),
    );
}

#[test]
fn task16_shared_store_adverse() {
    // Given: committed bytes and two conflicting proposals.
    let memory = Arc::new(SharedMemory::new());
    let mut device = SyncDevice::from_core(core(Arc::clone(&memory)));
    let initial = device.allocate_memory_region(4).expect("initial");
    device
        .write_memory(initial.base(), &[1, 2, 3, 4])
        .expect("write");
    let winner = device.stage_memory_regions(&[8]).expect("winner");
    let stale = device.stage_memory_regions(&[8]).expect("stale");
    device.commit_memory_stage(winner).expect("winner commit");
    let bytes_before = device.read_memory(initial.base(), 4).expect("bytes");
    let mappings_before = device.mapped_region_count();
    let generation_before = device.memory_generation();
    let regions_before = memory.regions();

    // When: stale, unmapped, and crossing operations are rejected.
    let stale_result = device.commit_memory_stage(stale);
    let unmapped = device.read_memory(MemoryAddress::new(0), 1);
    let crossing = device.write_memory(MemoryAddress::new(initial.base().raw() + 3), &[9, 9]);

    // Then: every rejection preserves bytes, mappings, generation, and runtime regions.
    assert!(matches!(
        stale_result,
        Err(PdmaMemoryError::StaleStage { .. })
    ));
    assert_eq!(unmapped, Err(PdmaMemoryError::Unmapped));
    assert_eq!(crossing, Err(PdmaMemoryError::CrossesRegion));
    let bytes_unchanged = device.read_memory(initial.base(), 4) == Ok(bytes_before);
    let mappings_unchanged = device.mapped_region_count() == mappings_before;
    let generation_unchanged = device.memory_generation() == generation_before;
    let regions_unchanged = memory.regions() == regions_before;
    assert!(bytes_unchanged && mappings_unchanged && generation_unchanged && regions_unchanged);
    println!(
        "bytes_unchanged={bytes_unchanged};mappings_unchanged={mappings_unchanged};generation_unchanged={generation_unchanged};regions_unchanged={regions_unchanged}"
    );
}

#[test]
fn injected_store_is_the_real_memory_access_surface() {
    // Given: a core consuming one injected store.
    let memory = Arc::new(SharedMemory::new());
    let mut device = SyncDevice::from_core(core(Arc::clone(&memory)));
    let region = device.allocate_memory_region(4).expect("allocation");

    // When: the injected runtime API writes the region.
    memory
        .write(region.memory_base(), ALIGNMENT, &[4, 3, 2, 1])
        .expect("runtime write");

    // Then: the device facade observes those same bytes.
    assert_eq!(device.read_memory(region.base(), 4), Ok(vec![4, 3, 2, 1]));
}
