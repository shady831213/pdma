//! Shared software FIFO using actual memory storage.
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use hardware_interface_schema::record_schema;
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing};
mod boundaries;
mod cancellation;
mod codec;
mod consume;
mod memory_cursor;
mod memory_error;
mod query;
mod transport;
pub use transport::CursorBus;

record_schema! {
    #[short_description = "Test entry"]
    #[description = "Software ring test payload."]
    record Item {
        alignment: 4, endian: Little,
        fields: {
            #[description = "Payload value."]
            value: u32,
        },
    }
}

#[tokio::test]
async fn fifo_without_cpu_context() -> Result<(), Box<dyn std::error::Error>> {
    // Given: independent software endpoints over one memory-backed FIFO.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, MemoryAddress::new(0), MemoryAddress::new(4));
    let mut producer = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
    let mut consumer = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
    // When: fill, drain, then reuse the ring.
    assert_eq!(consumer.peek(&io).await?, None);
    assert!(producer.try_push(&io, Item { value: 7 }).await?);
    assert!(producer.try_push(&io, Item { value: 8 }).await?);
    assert!(!producer.try_push(&io, Item { value: 9 }).await?);
    // Then: full does not retain data and values arrive in FIFO order.
    assert!(!producer.has_pending_push());
    assert_eq!(consumer.peek(&io).await?, Some(Item { value: 7 }));
    consumer.consume(&io).await?;
    assert_eq!(consumer.peek(&io).await?, Some(Item { value: 8 }));
    consumer.consume(&io).await?;
    assert!(producer.try_push(&io, Item { value: 9 }).await?);
    assert_eq!(consumer.peek(&io).await?, Some(Item { value: 9 }));
    Ok(())
}
