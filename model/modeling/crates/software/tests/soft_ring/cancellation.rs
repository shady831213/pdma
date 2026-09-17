use crate::{CursorBus, Item};
use hardware_interface_runtime::{
    CpuContextError, MemoryAccess, MemoryAlignment, MemoryRegion, SoftwareCpu,
};
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
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?)?;
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
            let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 1)?)?;
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
async fn resumed_future_rejects_wrong_cpu_before_publication()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a push suspended before its cursor publication on its owning CPU.
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
    let mut ring = cpu.run_main(async { SoftRing::new(config) }).await?;
    bus.pause.store(1, Ordering::SeqCst);
    let mut push = std::pin::pin!(ring.try_push(&io, Item { value: 1 }));
    cpu.run_main(poll_fn(|cx| {
        assert!(push.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    }))
    .await;
    // When: another CPU resumes that same future.
    let result = SoftwareCpu::new().run_main(push).await;
    // Then: the CPU guard rejects it before the inner transport is polled.
    assert!(matches!(
        result,
        Err(SoftRingError::Cpu(CpuContextError::Mismatch))
    ));
    assert_eq!(bus.writes.load(Ordering::SeqCst), 0);
    Ok(())
}
