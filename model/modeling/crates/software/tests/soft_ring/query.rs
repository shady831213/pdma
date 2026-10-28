use crate::{CursorBus, Item};
use hw_model_io::memory::MemoryAlignment;
use hw_model_io::memory::{MemoryRegion, SharedMemory};
use hw_model_swenv::SoftwareEnv;
use pdma_model_contract::ring::RingError;
use pdma_model_software::ring::{
    MemoryRingIo, RegisterRingIo, RingConfig, RingCursorRegisters, SoftRing, SoftRingError,
    SoftRingIo,
};
use std::{
    future::{Future, poll_fn},
    sync::atomic::Ordering,
    task::Poll,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn empty_query_reads_only_cursors_and_handles_wrap() -> TestResult {
    // Given: cursor storage exists but no entry storage is mapped.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    SoftwareEnv::new()
        .run_main(async {
            let ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            for (producer, consumer, empty) in [
                (0, 0, true),
                (1, 0, false),
                (2, 0, false),
                (0, u32::MAX, false),
                (u32::MAX, u32::MAX, true),
            ] {
                io.write_producer(producer).await?;
                io.write_consumer(consumer).await?;
                // When: querying empty, partially filled, full and wrapped snapshots.
                let observed = ring.is_empty(&io).await?;
                // Then: no entry access or cursor publication is needed.
                assert_eq!(observed, empty);
                assert_eq!(io.read_producer().await?, producer);
                assert_eq!(io.read_consumer().await?, consumer);
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn empty_query_rejects_invalid_snapshots() -> TestResult {
    // Given: cursor storage with an overrun or a consumer ahead of the producer.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = MemoryRingIo::new(&memory, 0.into(), 4.into());
    SoftwareEnv::new()
        .run_main(async {
            let ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            for (producer, consumer) in [(3, 0), (0, 1)] {
                io.write_producer(producer).await?;
                io.write_consumer(consumer).await?;
                // When: observing the ring through its query API.
                let result = ring.is_empty(&io).await;
                // Then: invalid state is not reported as an ordinary nonempty queue.
                assert!(matches!(
                    result,
                    Err(SoftRingError::Ring(RingError::Pointers))
                ));
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn empty_query_completes_outside_cpu_after_a_suspended_read() -> TestResult {
    // Given: an empty query suspended before reading its first cursor.
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &bus.memory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    let owner = SoftwareEnv::new();
    let config = RingConfig::<Item>::new(16, 2)?;
    let ring = owner.run_main(async { SoftRing::new(config) }).await;
    bus.pause_read.store(true, Ordering::SeqCst);
    let mut query = std::pin::pin!(ring.is_empty(&io));
    assert!(
        owner
            .run_main(poll_fn(|cx| Poll::Ready(query.as_mut().poll(cx))))
            .await
            .is_pending()
    );
    // When: the same query completes without a modeled CPU.
    assert!(query.await?);
    // Then: each cursor was read once and no write occurred.
    assert_eq!(bus.reads.load(Ordering::SeqCst), 2);
    assert_eq!(bus.writes.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn cancelled_empty_query_leaves_the_ring_usable() -> TestResult {
    // Given: a query suspended before its first cursor access.
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &bus.memory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    SoftwareEnv::new()
        .run_main(async {
            let ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            bus.pause_read.store(true, Ordering::SeqCst);
            {
                let mut query = std::pin::pin!(ring.is_empty(&io));
                // When: the caller cancels its pending observation.
                assert!(
                    poll_fn(|cx| Poll::Ready(query.as_mut().poll(cx)))
                        .await
                        .is_pending()
                );
            }
            // Then: a fresh query succeeds without publishing or leaving a retained operation.
            assert!(ring.is_empty(&io).await?);
            assert!(!ring.has_pending_push());
            assert_eq!(bus.writes.load(Ordering::SeqCst), 0);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}
