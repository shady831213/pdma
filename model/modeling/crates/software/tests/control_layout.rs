//! Shared control queue address validation and cursor access.

use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess};
use hw_model_io::memory::{MemoryRegion, SharedMemory};
use pdma_model_contract::control::{ControlQueue, ControlRingState};
use pdma_model_contract::ring::RingError;
use pdma_model_software::ring::SoftRingIo;
use std::sync::Arc;

#[tokio::test]
async fn shared_layout_targets_each_independent_memory_cursor()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one control queue with distinct command and response cursors.
    let memory = Arc::new(SharedMemory::new());
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        4096,
        MemoryAlignment::ONE,
    )?])?;
    let access = SharedMemoryAccess::from(memory);
    let layout = pdma_model_software::control::ControlQueueLayout::new(MemoryAddress::new(0x1000))?;
    // When: each side publishes through its shared layout view.
    layout.command_io(&access).write_producer(1).await?;
    layout.command_io(&access).write_consumer(2).await?;
    layout.response_io(&access).write_producer(3).await?;
    layout.response_io(&access).write_consumer(4).await?;
    // Then: the exact contract fields contain the independent values.
    for (ring, field, expected) in [
        (
            ControlQueue::COMMAND_OFFSET,
            ControlRingState::PRODUCER_OFFSET,
            1_u32,
        ),
        (
            ControlQueue::COMMAND_OFFSET,
            ControlRingState::CONSUMER_OFFSET,
            2,
        ),
        (
            ControlQueue::RESPONSE_OFFSET,
            ControlRingState::PRODUCER_OFFSET,
            3,
        ),
        (
            ControlQueue::RESPONSE_OFFSET,
            ControlRingState::CONSUMER_OFFSET,
            4,
        ),
    ] {
        let address = MemoryAddress::new(0x1000 + u64::try_from(ring + field)?);
        assert_eq!(
            access.read_buffer::<[u8; 4]>(address, MemoryAlignment::ONE)?,
            expected.to_le_bytes()
        );
    }
    Ok(())
}

#[test]
fn shared_layout_rejects_unaligned_or_overflowing_control_queue() {
    // Given: addresses that cannot hold the complete control queue record.
    for base in [1, u64::MAX - 7] {
        // When: shared software parses the queue address.
        let result =
            pdma_model_software::control::ControlQueueLayout::new(MemoryAddress::new(base));
        // Then: invalid address geometry is rejected before any cursor access.
        assert!(matches!(result, Err(RingError::Geometry)));
    }
}
