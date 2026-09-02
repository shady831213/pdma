use super::support;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount, decode_read, decode_write};
use pdma_model_device::AsyncDevice;

#[derive(Clone, Copy)]
enum MalformedRead {
    Read { offset: u32, width: MmioByteCount },
}

#[tokio::test]
async fn malformed_reads_preserve_all_observable_state() {
    // Given: distinct bank, runner, and DMA state snapshots.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let buffer = handle
        .allocate_buffer(4)
        .await
        .expect("DMA allocation should succeed");
    handle
        .write_buffer(buffer, 0, &[0xaa, 0xbb, 0xcc, 0xdd])
        .await
        .expect("DMA write should succeed");
    handle
        .write_mmio(0x108, &0x8877_6655_4433_2211_u64.to_le_bytes())
        .await
        .expect("bank seed should succeed");
    let before_runner = handle.snapshot().await;
    let before_dma_generation = handle.memory_generation().await;
    let before_dma = handle
        .read_buffer(buffer, 0, 4)
        .await
        .expect("DMA read should succeed");
    let before_bank = handle
        .read_mmio(0x108, 8)
        .await
        .expect("bank read should succeed");
    let before_results = support::result_latches(&handle)
        .await
        .expect("result latches should be readable");
    let accesses = [
        MalformedRead::Read {
            offset: 0,
            width: 1,
        },
        MalformedRead::Read {
            offset: 0,
            width: 2,
        },
        MalformedRead::Read {
            offset: 0,
            width: 16,
        },
        MalformedRead::Read {
            offset: 0,
            width: MmioByteCount::MAX,
        },
        MalformedRead::Read {
            offset: 0xffc,
            width: 8,
        },
        MalformedRead::Read {
            offset: 0x00c,
            width: 4,
        },
        MalformedRead::Read {
            offset: 0x006,
            width: 4,
        },
        MalformedRead::Read {
            offset: 0x004,
            width: 8,
        },
        MalformedRead::Read {
            offset: 0x000,
            width: 8,
        },
        MalformedRead::Read {
            offset: 0x010,
            width: 4,
        },
        MalformedRead::Read {
            offset: 0x020,
            width: 4,
        },
    ];

    // When: every malformed read reaches the public device access seam.
    for MalformedRead::Read { offset, width } in accesses {
        let actual = handle.read_mmio(offset, width).await;

        // Then: error identity is contract-owned and every observable remains unchanged.
        assert_eq!(actual, decode_read(offset, width).map(|_| Vec::new()));
        assert_eq!(handle.snapshot().await, before_runner);
        assert_eq!(handle.memory_generation().await, before_dma_generation);
        assert_eq!(
            handle.read_buffer(buffer, 0, 4).await,
            Ok(before_dma.clone())
        );
        assert_eq!(handle.read_mmio(0x108, 8).await, Ok(before_bank.clone()));
        assert_eq!(
            support::result_latches(&handle).await,
            Ok(before_results.clone())
        );
    }
}

#[tokio::test]
async fn malformed_writes_preserve_all_observable_state() {
    // Given: a seeded register bank and its device snapshot.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let seed = 0x0123_4567_89ab_cdef_u64.to_le_bytes();
    handle
        .write_mmio(0x108, &seed)
        .await
        .expect("bank seed should succeed");
    let before_runner = handle.snapshot().await;
    let before_dma_generation = handle.memory_generation().await;
    let before_bank = handle
        .read_mmio(0x108, 8)
        .await
        .expect("bank read should succeed");
    let before_results = support::result_latches(&handle)
        .await
        .expect("result latches should be readable");
    let oversized = [0_u8; 256];
    let writes: [(u32, &[u8]); 8] = [
        (0x108, &[0]),
        (0x108, &[0, 0]),
        (0x108, &[0; 16]),
        (0x108, &oversized),
        (0xffc, &[0; 8]),
        (0x006, &[0; 4]),
        (0x000, &[0; 4]),
        (0x128, &[1, 0, 0, 0]),
    ];

    // When: every malformed write reaches the public device access seam.
    for (offset, bytes) in writes {
        let actual = handle.write_mmio(offset, bytes).await;

        // Then: the exact contract error is returned before any mutation.
        assert_eq!(actual, decode_write(offset, bytes).map(|_| ()));
        assert_eq!(handle.snapshot().await, before_runner);
        assert_eq!(handle.memory_generation().await, before_dma_generation);
        assert_eq!(handle.read_mmio(0x108, 8).await, Ok(before_bank.clone()));
        assert_eq!(
            support::result_latches(&handle).await,
            Ok(before_results.clone())
        );
    }

    assert!(matches!(
        handle.write_mmio(0x128, &[1, 0, 0, 0]).await,
        Err(MmioAccessError::ReservedBits {
            offset: 0x128,
            bits: 1
        })
    ));
}
