use super::support::{self, TestResult};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_device::PdmaMemoryError;

#[tokio::test]
async fn checked_memory_errors_reject_only_the_attempted_access() -> TestResult {
    // Given: adjacent committed regions and a completed sentinel write.
    let handle = support::new_handle();
    let first = handle.allocate_memory_region(64).await?;
    let second = handle.allocate_memory_region(64).await?;
    handle.write_memory(first.base(), &[0x71; 64]).await?;
    let before = handle.snapshot().await;
    // When: writes begin outside the mappings or cross a mapped region boundary.
    let unmapped = handle.write_memory(DmaAddress::new(0), &[1]).await;
    let crossing = handle
        .write_memory(first.base().checked_add(63)?, &[2, 3])
        .await;
    // Then: typed memory errors preserve mapping contents and notification state.
    assert_eq!(unmapped, Err(PdmaMemoryError::Unmapped));
    assert_eq!(crossing, Err(PdmaMemoryError::CrossesRegion));
    assert_eq!(handle.read_memory(first.base(), 64).await?, vec![0x71; 64]);
    assert_eq!(handle.read_memory(second.base(), 64).await?, vec![0; 64]);
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}
