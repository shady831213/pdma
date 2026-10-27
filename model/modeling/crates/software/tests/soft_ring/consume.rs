use crate::{CursorBus, Item};
use hardware_interface_memory::{MemoryAlignment, MemoryRegion};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_software::ring::{
    RegisterRingIo, RingConfig, RingCursorRegisters, SoftRing, SoftRingIo,
};

use std::{
    future::{Future, poll_fn},
    sync::atomic::Ordering,
    task::Poll,
};

#[tokio::test]
async fn cancelled_consume_before_commit_retains_entry() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a cached entry and a transport suspended before committing consumption.
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
            ring.try_push(&io, Item { value: 29 }).await?;
            assert_eq!(ring.peek(&io).await?, Some(Item { value: 29 }));
            bus.pause.store(1, Ordering::SeqCst);
            // When: cancellation drops the pending consumer write.
            {
                let mut consume = std::pin::pin!(ring.consume(&io));
                poll_fn(|cx| {
                    assert!(consume.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
            }
            // Then: entry stays visible until the retained consume is resumed.
            assert_eq!(io.read_consumer().await?, 0);
            assert_eq!(ring.peek(&io).await?, Some(Item { value: 29 }));
            ring.consume(&io).await?;
            assert_eq!(io.read_consumer().await?, 1);
            assert_eq!(bus.writes.load(Ordering::SeqCst), 2);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}
