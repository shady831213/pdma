use crate::{CursorBus, Item};
use hardware_interface_memory::{MemoryAccess, MemoryAlignment, MemoryRegion};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_software::ring::{
    RegisterRingIo, RingConfig, RingCursorRegisters, SoftRing, SoftRingError, SoftRingIo,
};

use std::{
    future::{Future, poll_fn},
    sync::atomic::Ordering,
    task::Poll,
};

async fn push_after_cancel(pause: u8) -> Result<(), Box<dyn std::error::Error>> {
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &bus.memory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    SoftwareCpu::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
            bus.pause.store(pause, Ordering::SeqCst);
            {
                let mut push = std::pin::pin!(ring.try_push(&io, Item { value: 23 }));
                poll_fn(|cx| {
                    assert!(push.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
            }
            assert!(matches!(
                ring.try_push(&io, Item { value: 99 }).await,
                Err(SoftRingError::Busy)
            ));
            assert_eq!(ring.is_empty(&io).await?, pause == 1);
            assert!(ring.has_pending_push());
            ring.resume_push(&io).await?;
            assert!(!ring.has_pending_push());
            assert_eq!(io.read_producer().await?, 1);
            assert_eq!(bus.writes.load(Ordering::SeqCst), 1);
            assert_eq!(ring.peek(&io).await?, Some(Item { value: 23 }));
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn resumes_push_before_cursor_commit() -> Result<(), Box<dyn std::error::Error>> {
    // Given/When/Then: cancellation before publication retains exactly one entry.
    push_after_cancel(1).await
}

#[tokio::test]
async fn resumes_push_after_cursor_commit_without_duplicate()
-> Result<(), Box<dyn std::error::Error>> {
    // Given/When/Then: a committed publication is reconciled without another write.
    push_after_cancel(2).await
}

#[tokio::test]
async fn cached_peek_survives_slot_reuse_after_consumer_commit()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one cached entry in a single-slot register-backed ring.
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &bus.memory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    SoftwareCpu::new()
        .run_main(async {
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 1)?);
            ring.try_push(&io, Item { value: 11 }).await?;
            assert_eq!(ring.peek(&io).await?, Some(Item { value: 11 }));
            bus.pause.store(2, Ordering::SeqCst);
            // When: consume commits, suspends, and its freed slot is overwritten.
            {
                let mut consume = std::pin::pin!(ring.consume(&io));
                poll_fn(|cx| {
                    assert!(consume.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
            }
            bus.memory
                .write(16.into(), MemoryAlignment::ONE, &88u32.to_le_bytes())?;
            // Then: retry sees cached data and acknowledges the committed consume once.
            assert!(ring.is_empty(&io).await?);
            assert_eq!(ring.peek(&io).await?, Some(Item { value: 11 }));
            ring.consume(&io).await?;
            assert_eq!(io.read_consumer().await?, 1);
            assert_eq!(ring.peek(&io).await?, None);
            assert_eq!(bus.writes.load(Ordering::SeqCst), 2);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}

#[tokio::test]
async fn suspended_publication_completes_on_another_cpu() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a push suspended before publication on one CPU.
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &bus.memory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    let cpu = SoftwareCpu::new();
    let config = RingConfig::<Item>::new(16, 2)?;
    let mut ring = cpu.run_main(async { SoftRing::new(config) }).await;
    bus.pause.store(3, Ordering::SeqCst);
    {
        let mut push = std::pin::pin!(ring.try_push(&io, Item { value: 1 }));
        assert!(
            cpu.run_main(poll_fn(|cx| Poll::Ready(push.as_mut().poll(cx))))
                .await
                .is_pending()
        );
        // When: a different CPU completes the same future.
        assert!(SoftwareCpu::new().run_main(push).await?);
    }
    // Then: exactly one entry was published; cached data and consumption work outside a CPU.
    assert!(!ring.has_pending_push());
    assert_eq!(ring.peek(&io).await?, Some(Item { value: 1 }));
    assert_eq!(ring.peek(&io).await?, Some(Item { value: 1 }));
    ring.consume(&io).await?;
    let bytes: [u8; 4] = bus.memory.read_typed(16.into(), MemoryAlignment::ONE)?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    assert_eq!(io.read_producer().await?, 1);
    assert_eq!(io.read_consumer().await?, 1);
    assert_eq!(bus.writes.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn cancelled_publication_rejects_an_unrelated_producer_cursor()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: publication was canceled after writing its entry but before its cursor.
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 64, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &bus.memory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
    bus.pause.store(1, Ordering::SeqCst);
    {
        let mut push = std::pin::pin!(ring.try_push(&io, Item { value: 23 }));
        assert!(
            poll_fn(|cx| Poll::Ready(push.as_mut().poll(cx)))
                .await
                .is_pending()
        );
    }
    // When: another producer changes the retained operation's cursor.
    io.write_producer(7).await?;
    // Then: resume rejects stale state without overwriting that cursor or accepting new data.
    assert!(matches!(
        ring.resume_push(&io).await,
        Err(SoftRingError::CursorChanged)
    ));
    assert!(matches!(
        ring.try_push(&io, Item { value: 99 }).await,
        Err(SoftRingError::Busy)
    ));
    assert!(ring.has_pending_push());
    assert_eq!(io.read_producer().await?, 7);
    assert_eq!(bus.writes.load(Ordering::SeqCst), 1);
    let bytes: [u8; 4] = bus.memory.read_typed(16.into(), MemoryAlignment::ONE)?;
    assert_eq!(u32::from_le_bytes(bytes), 23);
    Ok(())
}
