#![allow(missing_docs)]

#[path = "mmio_parity/irq.rs"]
mod irq;
#[path = "mmio_parity/queue.rs"]
mod queue;

use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount, MmioResult};
use pdma_model_contract::{
    DeviceEpoch, DeviceRunState, DeviceStatus, DiscoveryRequest, WholeDeviceReset,
};
use pdma_model_device::AsyncDevice;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) async fn write_u32(
    handle: &pdma_model_device::AsyncDeviceHandle,
    offset: u32,
    value: u32,
) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub(crate) async fn write_u64(
    handle: &pdma_model_device::AsyncDeviceHandle,
    offset: u32,
    value: u64,
) -> TestResult {
    handle.write_mmio(offset, &value.to_le_bytes()).await?;
    Ok(())
}

pub(crate) async fn read_u32(
    handle: &pdma_model_device::AsyncDeviceHandle,
    offset: u32,
) -> TestResult<u32> {
    let bytes = handle.read_mmio(offset, MmioByteCount::from(4_u8)).await?;
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| "four bytes")?,
    ))
}

pub(crate) async fn read_u64(
    handle: &pdma_model_device::AsyncDeviceHandle,
    offset: u32,
) -> TestResult<u64> {
    let bytes = handle.read_mmio(offset, MmioByteCount::from(8_u8)).await?;
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| "eight bytes")?,
    ))
}

#[tokio::test(flavor = "current_thread")]
async fn raw_endpoint_preserves_little_endian_bytes_and_exact_width_errors() -> TestResult {
    // Given: one untouched device and literal malformed write slices.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();

    // When: constants are read and arbitrary byte lengths cross the public raw endpoint.
    let magic = handle.read_mmio(0x000, 4).await?;
    let malformed = handle.write_mmio(0x010, &[0; 16]).await;
    let split = handle.write_mmio(0x010, &[0; 4]).await;

    // Then: bytes are literal LE lanes and widths retain their full u128 values.
    assert_eq!(magic, [0x50, 0x44, 0x4d, 0x41]);
    assert_eq!(
        malformed,
        Err(MmioAccessError::WrongWidth {
            offset: 0x010,
            expected: 0,
            actual: MmioByteCount::from(16_u8),
        })
    );
    assert_eq!(
        split,
        Err(MmioAccessError::WrongWidth {
            offset: 0x010,
            expected: MmioByteCount::from(8_u8),
            actual: MmioByteCount::from(4_u8),
        })
    );
    assert_eq!(read_u64(&handle, 0x010).await?, 0);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn typed_and_raw_discovery_and_current_status_match() -> TestResult {
    // Given: equivalent untouched devices and one validated discovery request.
    let (typed_device, _typed_worker) = AsyncDevice::queue_port();
    let (raw_device, _raw_worker) = AsyncDevice::queue_port();
    let typed = typed_device.handle();
    let raw = raw_device.handle();
    let request = DiscoveryRequest::try_new(1, 0)?;

    // When: typed and raw device endpoints independently discover and report status.
    let typed_discovery = typed.discover(request);
    let typed_status = typed.current_status().await;
    write_u32(&raw, 0x008, 1).await?;
    write_u64(&raw, 0x010, 0).await?;
    write_u32(&raw, 0x020, 1).await?;
    let discovery_bytes = (
        raw.read_mmio(0x040, 4).await?,
        raw.read_mmio(0x048, 8).await?,
    );
    write_u32(&raw, 0x020, 3).await?;
    let raw_status = DeviceStatus::new(
        DeviceEpoch::new(read_u64(&raw, 0x050).await?),
        DeviceRunState::try_from(u16::try_from(read_u32(&raw, 0x044).await?)?)?,
    );

    // Then: both real surfaces return the same discovery and current status values.
    assert_eq!(
        discovery_bytes.0,
        u32::from(typed_discovery.version()).to_le_bytes()
    );
    assert_eq!(
        discovery_bytes.1,
        typed_discovery.features().bits().to_le_bytes()
    );
    assert_eq!(read_u32(&raw, 0x024).await?, MmioResult::Ok.code());
    assert_eq!(typed_status, raw_status);
    assert_eq!(typed_status.run_state(), DeviceRunState::Idle);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn raw_reset_matches_typed_reset_outcome() -> TestResult {
    // Given: two independent devices at the same initial epoch.
    let (typed_device, _typed_worker) = AsyncDevice::queue_port();
    let (raw_device, _raw_worker) = AsyncDevice::queue_port();
    let raw_before = raw_device.handle().snapshot().await;

    // When: one resets through the typed endpoint and one through raw MMIO.
    let typed = typed_device
        .handle()
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    write_u64(&raw_device.handle(), 0x018, 0).await?;
    write_u32(&raw_device.handle(), 0x020, 4).await?;

    // Then: the raw result and post-reset epoch exactly match the typed outcome.
    assert_eq!(typed, DeviceEpoch::new(1));
    assert_eq!(
        read_u32(&raw_device.handle(), 0x024).await?,
        MmioResult::Ok.code()
    );
    assert_eq!(
        raw_device.handle().read_mmio(0x050, 8).await?,
        1_u64.to_le_bytes()
    );
    assert!(raw_device.handle().snapshot().await.generation() > raw_before.generation());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn stale_raw_reset_matches_typed_error_without_publication() -> TestResult {
    // Given: typed and raw devices both advanced to epoch one.
    let (typed_device, _typed_worker) = AsyncDevice::queue_port();
    let (raw_device, _raw_worker) = AsyncDevice::queue_port();
    let typed = typed_device.handle();
    let raw = raw_device.handle();
    typed
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    write_u64(&raw, 0x018, 0).await?;
    write_u32(&raw, 0x020, 4).await?;
    let before = raw.snapshot().await;

    // When: both surfaces receive the stale initial epoch again.
    let typed_error = typed
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await;
    write_u64(&raw, 0x018, 0).await?;
    write_u32(&raw, 0x020, 4).await?;

    // Then: typed identity and raw bytes agree, and rejection publishes no generation.
    assert_eq!(
        typed_error,
        Err(pdma_model_device::BusControlError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    );
    assert_eq!(read_u32(&raw, 0x024).await?, MmioResult::StaleEpoch.code());
    assert_eq!(
        (
            read_u64(&raw, 0x028).await?,
            read_u64(&raw, 0x030).await?,
            read_u64(&raw, 0x038).await?,
            read_u64(&raw, 0x050).await?,
        ),
        (1, 0, 0, 1)
    );
    assert_eq!(raw.snapshot().await, before);
    Ok(())
}
