use super::support::{self, TestResult};
use hardware_interface_runtime::{MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::abi::DmaAddress;

#[tokio::test]
async fn checked_memory_errors_reject_only_the_attempted_access() -> TestResult {
    // Given: adjacent committed regions and a completed sentinel write.
    let handle = support::new_handle();
    let first = handle.allocate(64)?;
    let second = handle.allocate(64)?;
    handle.write_memory(first, &[0x71; 64]).await?;
    let before = handle.snapshot().await;
    // When: writes begin outside the mappings or cross a mapped region boundary.
    let unmapped = handle.write_memory(DmaAddress::new(0), &[1]).await;
    let crossing = handle.write_memory(first.checked_add(63)?, &[2, 3]).await;
    // Then: typed memory errors preserve mapping contents and notification state.
    assert_eq!(
        unmapped,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0)
        })
    );
    assert_eq!(
        crossing,
        Err(MemoryAccessError::CrossesRegion {
            address: MemoryAddress::new(first.raw() + 63),
            length: 2,
            region: MemoryRegion::try_new(
                MemoryAddress::new(first.raw()),
                64,
                MemoryAlignment::try_new(64)?
            )?,
        })
    );
    assert_eq!(handle.read_memory(first, 64).await?, vec![0x71; 64]);
    assert_eq!(handle.read_memory(second, 64).await?, vec![0; 64]);
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}
