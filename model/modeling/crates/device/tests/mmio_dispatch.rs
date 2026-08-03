//! Device MMIO dispatcher integration coverage.

use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::abi::{DeviceEpoch, WholeDeviceReset};
use pdma_model_device::AsyncDevice;

#[path = "mmio_dispatch/malformed.rs"]
mod malformed;
#[path = "mmio_dispatch/source.rs"]
mod source;
#[path = "mmio_dispatch/support.rs"]
/// Shared observability helpers for malformed-access cases.
pub mod support;

#[tokio::test]
async fn register_families_route_when_access_is_valid() {
    // Given: a device whose MMIO banks are at their construction reset values.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();

    // When: representative global, queue, and IRQ staging registers are written.
    handle
        .write_mmio(0x010, &0x0123_4567_89ab_cdef_u64.to_le_bytes())
        .await
        .expect("global staging write should succeed");
    handle
        .write_mmio(0x108, &0x1122_3344_5566_7788_u64.to_le_bytes())
        .await
        .expect("queue staging write should succeed");
    handle
        .write_mmio(0x214, &1_u32.to_le_bytes())
        .await
        .expect("IRQ staging write should succeed");

    // Then: each family returns the exact byte-bearing value through the same raw seam.
    assert_eq!(
        handle.read_mmio(0x010, 8).await,
        Ok(0x0123_4567_89ab_cdef_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x108, 8).await,
        Ok(0x1122_3344_5566_7788_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x214, 4).await,
        Ok(1_u32.to_le_bytes().to_vec())
    );
}

#[tokio::test]
async fn result_and_reset_banks_round_trip_when_commands_are_not_implemented() {
    // Given: a newly constructed device register bank.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();

    // When: representative command writes route into the non-semantic family scaffolds.
    handle
        .write_mmio(0x020, &1_u32.to_le_bytes())
        .await
        .expect("global command scaffold should accept the write");
    handle
        .write_mmio(0x168, &1_u32.to_le_bytes())
        .await
        .expect("queue command scaffold should accept the write");
    handle
        .write_mmio(0x220, &1_u32.to_le_bytes())
        .await
        .expect("IRQ command scaffold should accept the write");
    let magic = handle.read_mmio(0x000, 4).await;
    let version = handle.read_mmio(0x004, 4).await;
    let global_result = handle.read_mmio(0x024, 4).await;
    let queue_result = handle.read_mmio(0x16c, 4).await;
    let irq_result = handle.read_mmio(0x224, 4).await;

    // Then: constants and construction reset values match the canonical encodings.
    assert_eq!(magic, Ok(0x414d_4450_u32.to_le_bytes().to_vec()));
    assert_eq!(version, Ok(1_u32.to_le_bytes().to_vec()));
    let unsupported_version = MmioResult::UnsupportedVersion.code().to_le_bytes().to_vec();
    assert_eq!(global_result, Ok(unsupported_version));
    let not_executed = MmioResult::NotExecuted.code().to_le_bytes().to_vec();
    assert_eq!(queue_result, Ok(not_executed.clone()));
    assert_eq!(irq_result, Ok(not_executed));
    assert_eq!(
        handle.read_mmio(0x028, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x188, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x240, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
}

#[tokio::test]
async fn whole_device_reset_restores_register_banks() {
    // Given: nonzero values in all three mutable register families.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    handle
        .write_mmio(0x010, &u64::MAX.to_le_bytes())
        .await
        .expect("global staging write should succeed");
    handle
        .write_mmio(0x108, &u64::MAX.to_le_bytes())
        .await
        .expect("queue staging write should succeed");
    handle
        .write_mmio(0x214, &1_u32.to_le_bytes())
        .await
        .expect("IRQ staging write should succeed");

    // When: the existing typed whole-device reset succeeds.
    let epoch = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::new(0)))
        .await
        .expect("whole-device reset should succeed");

    // Then: the epoch advances and every register family returns its reset values.
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(
        handle.read_mmio(0x010, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x108, 8).await,
        Ok(0_u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        handle.read_mmio(0x214, 4).await,
        Ok(0_u32.to_le_bytes().to_vec())
    );
}
