use hardware_interface_runtime::{RegisterAccess, SharedMemory};
use pdma_model_contract::abi::registers::{
    GlobalRegisters, MmioAccessError, QueueRegisters, queue_offset,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, MmioRegisterError, PdmaMemoryProvisioning};
use std::sync::Arc;

#[tokio::test]
async fn malformed_reads_preserve_all_observable_state() {
    // Given: mapped bytes and a distinct queue configuration register.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let region = handle.allocate_memory_region(4).await.expect("region");
    handle
        .write_memory(region.base(), &[0xaa, 0xbb, 0xcc, 0xdd])
        .await
        .expect("bytes");
    let base = queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("base offset");
    endpoint
        .write(base, &0x8877_6655_4433_2211_u64.to_le_bytes())
        .await
        .expect("seed");
    let snapshot = handle.snapshot().await;
    let generation = handle.memory_generation().await;
    let reset = u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset");
    let accesses = [
        (
            0,
            1,
            MmioAccessError::WrongWidth {
                offset: 0,
                expected: 0,
                actual: 1,
            },
        ),
        (
            6,
            4,
            MmioAccessError::Misaligned {
                offset: 6,
                width: 4,
            },
        ),
        (
            0,
            8,
            MmioAccessError::WrongWidth {
                offset: 0,
                expected: 4,
                actual: 8,
            },
        ),
        (
            reset,
            4,
            MmioAccessError::ReadFromWriteOnly { offset: reset },
        ),
    ];

    // When: malformed reads reach the public register trait.
    for (offset, width, expected) in accesses {
        let mut bytes = vec![0; width];
        assert_eq!(
            endpoint.read(offset, &mut bytes).await,
            Err(MmioRegisterError::Access(expected))
        );
    }

    // Then: register state, runtime accounting and committed memory survive.
    assert_eq!(handle.snapshot().await, snapshot);
    assert_eq!(handle.memory_generation().await, generation);
    assert_eq!(
        handle.read_memory(region.base(), 4).await.expect("bytes"),
        [0xaa, 0xbb, 0xcc, 0xdd]
    );
    let mut bytes = [0; 8];
    endpoint.read(base, &mut bytes).await.expect("seed");
    assert_eq!(u64::from_le_bytes(bytes), 0x8877_6655_4433_2211);
}

#[tokio::test]
async fn malformed_writes_preserve_registers_and_accounting() {
    // Given: a distinct queue configuration register and stable runtime snapshot.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let base = queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("base");
    let enable = queue_offset(0, QueueRegisters::ENABLE.offset()).expect("enable");
    let seed = 0x0123_4567_89ab_cdef_u64.to_le_bytes();
    endpoint.write(base, &seed).await.expect("seed");
    let snapshot = handle.snapshot().await;
    let generation = handle.memory_generation().await;
    let writes: [(u32, &[u8], MmioAccessError); 4] = [
        (
            base,
            &[0],
            MmioAccessError::WrongWidth {
                offset: base,
                expected: 0,
                actual: 1,
            },
        ),
        (
            6,
            &[0; 4],
            MmioAccessError::Misaligned {
                offset: 6,
                width: 4,
            },
        ),
        (0, &[0; 4], MmioAccessError::WriteToReadOnly { offset: 0 }),
        (
            enable,
            &[2, 0, 0, 0],
            MmioAccessError::ReservedBits {
                offset: enable,
                bits: 2,
            },
        ),
    ];

    // When: malformed writes reach the public register trait.
    for (offset, bytes, expected) in writes {
        assert_eq!(
            endpoint.write(offset, bytes).await,
            Err(MmioRegisterError::Access(expected))
        );
    }

    // Then: none of the writes reaches its mutation point.
    assert_eq!(handle.snapshot().await, snapshot);
    assert_eq!(handle.memory_generation().await, generation);
    let mut bytes = [0; 8];
    endpoint.read(base, &mut bytes).await.expect("seed");
    assert_eq!(bytes, seed);
}
