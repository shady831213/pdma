use crate::Item;
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_software::SoftwareEnv;
use pdma_model_contract::ring::RingError;
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing, SoftRingError, SoftRingIo};

#[tokio::test]
async fn wraps_cursors_without_losing_fifo_order() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an empty ring at the last u32 cursor.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    io.write_producer(u32::MAX).await?;
    io.write_consumer(u32::MAX).await?;
    SoftwareEnv::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            // When: an entry crosses the monotonic cursor boundary.
            ring.try_push(&io, Item { value: 42 }).await?;
            assert_eq!(ring.peek(&io).await?, Some(Item { value: 42 }));
            ring.consume(&io).await?;
            // Then: both cursors wrap to zero and the ring is empty.
            assert_eq!(io.read_producer().await?, 0);
            assert_eq!(io.read_consumer().await?, 0);
            assert_eq!(ring.peek(&io).await?, None);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn overrun_is_rejected_without_accessing_entries() -> Result<(), Box<dyn std::error::Error>> {
    // Given: pointers beyond capacity and no mapped entry storage.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    io.write_producer(3).await?;
    SoftwareEnv::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            // When/Then: the ring reports pointer failure before entry-memory failure.
            assert!(matches!(
                ring.peek(&io).await,
                Err(SoftRingError::Ring(RingError::Pointers))
            ));
            assert_eq!(io.read_consumer().await?, 0);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn failed_entry_write_does_not_publish() -> Result<(), Box<dyn std::error::Error>> {
    // Given: valid cursors but missing entry memory.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    SoftwareEnv::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            // When: the synchronous data write fails.
            assert!(matches!(
                ring.try_push(&io, Item { value: 7 }).await,
                Err(SoftRingError::Memory(_))
            ));
            // Then: producer remains unpublished and retry retains the original entry.
            assert_eq!(io.read_producer().await?, 0);
            memory.add_regions(&[MemoryRegion::try_new(16.into(), 8, MemoryAlignment::ONE)?])?;
            ring.resume_push(&io).await?;
            let bytes: [u8; 4] = memory.read_buffer(16.into(), MemoryAlignment::ONE)?;
            assert_eq!(u32::from_le_bytes(bytes), 7);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}
