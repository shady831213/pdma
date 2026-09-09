#![allow(missing_docs)]

mod irq;

use super::mmio_queue::support::*;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{GlobalRegisters, MmioAccessError, QueueRegisters};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::MmioRegisterError;

#[tokio::test]
async fn raw_endpoint_preserves_little_endian_bytes_and_exact_width_errors() -> TestResult {
    // Given: an untouched endpoint and exact generated register offsets.
    let (device, _worker, _allocator) = device(ResourceConfig::default());
    let endpoint = device.handle().register_endpoint();
    let base = q(QueueRegisters::SQ_BASE.offset())?;
    let mut magic = [0; 4];
    // When: discovery is read and split/oversized writes target a 64-bit register.
    endpoint
        .read(u32::try_from(GlobalRegisters::MAGIC.offset())?, &mut magic)
        .await?;
    let oversized = endpoint.write(base, &[0; 16]).await;
    let split = endpoint.write(base, &[0; 4]).await;
    // Then: literal little-endian bytes are retained and wrong widths cannot mutate state.
    assert_eq!(magic, [0x50, 0x44, 0x4d, 0x41]);
    assert_eq!(
        oversized,
        Err(MmioRegisterError::Access(MmioAccessError::WrongWidth {
            offset: base,
            expected: 0,
            actual: 16,
        }))
    );
    assert_eq!(
        split,
        Err(MmioRegisterError::Access(MmioAccessError::WrongWidth {
            offset: base,
            expected: 8,
            actual: 4,
        }))
    );
    assert_eq!(read_u64(&endpoint, base).await?, 0);
    Ok(())
}
