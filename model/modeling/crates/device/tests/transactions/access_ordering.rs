#![allow(missing_docs, reason = "integration test crate")]

use super::ordered_access::support;
use hardware_interface_runtime::{AccessOrigin, RegisterAccess};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::semantic::DeviceEpoch;

mod adverse;
mod happy;

#[tokio::test]
async fn primitive_access_adverse_surface() -> support::TestResult {
    adverse::run().await
}

#[tokio::test]
async fn cloned_endpoints_share_observation_order() -> support::TestResult {
    // Given: two endpoints sharing one observation gate.
    let (handle, observer) = support::observed_handle();
    let endpoint = handle.register_endpoint();
    // When: each clone performs an independent request-register write.
    endpoint.write(0x100, &0_u64.to_le_bytes()).await?;
    endpoint.clone().write(0x108, &1_u64.to_le_bytes()).await?;
    // Then: every access has its own contiguous sequence and direct origin.
    let observations: [_; 2] = observer
        .snapshot()
        .try_into()
        .map_err(|_| "exactly two observations required")?;
    assert_eq!([observations[0].sequence, observations[1].sequence], [0, 1]);
    assert_eq!(
        [observations[0].offset, observations[1].offset],
        [0x100, 0x108]
    );
    assert!(
        observations
            .iter()
            .all(|item| item.origin == AccessOrigin::Direct)
    );
    Ok(())
}

#[tokio::test]
async fn racing_register_accesses_reserve_global_commit_order() -> support::TestResult {
    // Given: two direct accesses contending for the shared endpoint gate.
    let (handle, observer) = support::observed_handle();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let first_barrier = std::sync::Arc::clone(&barrier);
    let second_barrier = std::sync::Arc::clone(&barrier);
    let first_endpoint = handle.register_endpoint();
    let second_endpoint = first_endpoint.clone();
    // When: both register futures contend from the same release point.
    let first = std::thread::spawn(move || -> support::TestResult {
        first_barrier.wait();
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(first_endpoint.write(0x100, &3_u64.to_le_bytes()))?;
        Ok(())
    });
    let second = std::thread::spawn(move || -> support::TestResult {
        second_barrier.wait();
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(second_endpoint.write(0x108, &4_u64.to_le_bytes()))?;
        Ok(())
    });
    barrier.wait();
    first.join().map_err(|_| "first thread panicked")??;
    second.join().map_err(|_| "second thread panicked")??;
    // Then: either winner is valid and the observed writes remain complete and ordered.
    let observations: [_; 2] = observer
        .snapshot()
        .try_into()
        .map_err(|_| "exactly two raced observations required")?;
    assert_eq!([observations[0].sequence, observations[1].sequence], [0, 1]);
    assert_ne!(observations[0].offset, observations[1].offset);
    assert!(
        observations
            .iter()
            .all(|item| item.origin == AccessOrigin::Direct)
    );
    assert_eq!(handle.read_mmio(0x100, 8).await?, 3_u64.to_le_bytes());
    assert_eq!(handle.read_mmio(0x108, 8).await?, 4_u64.to_le_bytes());
    Ok(())
}

#[tokio::test]
async fn memory_writer_serializes_with_standalone_mapping_commit() -> support::TestResult {
    // Given/When: a runtime writer holds the store while standalone mapping commits.
    let serialized = adverse::writer::run().await?;
    // Then: both memory-only commits survive before queue activation.
    assert!(serialized);
    Ok(())
}
