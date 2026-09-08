#![allow(missing_docs)]

mod states;

use super::mmio_queue::support::*;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::GlobalRegisters;
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::MmioRegisterError;

#[tokio::test]
async fn global_registers_expose_canonical_discovery_and_limits_without_mutation() -> TestResult {
    // Given: a fresh device with canonical data ABI discovery and limits.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let before = handle.snapshot().await;
    // When: software reads the direct discovery and limits registers.
    for (offset, expected) in [
        (GlobalRegisters::MAGIC.offset(), 0x414d_4450),
        (GlobalRegisters::MMIO_VERSION.offset(), 2),
        (GlobalRegisters::ABI_VERSION.offset(), 1),
        (GlobalRegisters::MAX_RING_DEPTH.offset(), u32::MAX),
        (
            GlobalRegisters::MAX_INDIRECT_DESTINATIONS.offset(),
            u32::MAX,
        ),
        (GlobalRegisters::MAX_TRANSFER_BYTES.offset(), u32::MAX),
        (GlobalRegisters::MAX_STRIDED_ROWS.offset(), u32::MAX),
        (GlobalRegisters::MAX_STRIDE.offset(), u32::MAX),
    ] {
        assert_eq!(read_u32(&endpoint, u32::try_from(offset)?).await?, expected);
    }
    assert_eq!(
        read_u64(
            &endpoint,
            u32::try_from(GlobalRegisters::FEATURES.offset())?
        )
        .await?,
        0xff
    );
    // Then: malformed accesses fail without changing lifecycle state or read-only discovery.
    assert!(matches!(
        endpoint
            .read(u32::try_from(GlobalRegisters::MAGIC.offset())?, &mut [0; 8])
            .await,
        Err(MmioRegisterError::Access(_))
    ));
    assert!(matches!(
        write_u32(
            &endpoint,
            u32::try_from(GlobalRegisters::ABI_VERSION.offset())?,
            2
        )
        .await,
        Err(MmioRegisterError::Access(_))
    ));
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}
