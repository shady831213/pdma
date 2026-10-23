use super::{PlatformTimer, TestResult, fence, resume, rig, timestamp};
use crate::command_operations::{
    faults::rig_channels,
    support::{EVENT_PRODUCER, Rig},
};
use pdma_model_contract::dma::{
    ChannelId, Command, ContextId, DmaAdmin, DmaDiscovery, TimestampCommand, WordWrite,
};
use pdma_model_dma::MaintenanceError;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[test]
fn parked_timestamp_does_not_block_peer_channel() -> TestResult {
    // Given: two physical channels share one engine and timer.
    let mut rig = rig_channels(0x9000, 2)?;
    rig.dma = rig
        .dma
        .with_timer(Arc::new(PlatformTimer(AtomicU64::new(17))));
    rig.submit(0, &[timestamp(1, 0x9000), fence(0x8000)])?;
    rig.submit(1, &[timestamp(2, 0x9010), fence(0x8010)])?;
    // When: the first channel parks on its Timestamp target.
    rig.drive()?;
    // Then: the peer completes, while the parked channel and its successor remain untouched.
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.word(0x9010)?, 17);
    assert_eq!(rig.word(0x8010)?, 0xfeed);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    assert_eq!(rig.register(1, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn reset_discards_sample_but_preserves_external_timer() -> TestResult {
    // Given: a parked timestamp owns a sample from an external timer.
    let mut rig = rig(0x9000)?;
    let timer = Arc::new(PlatformTimer(AtomicU64::new(17)));
    rig.dma = rig.dma.with_timer(timer.clone());
    rig.submit(0, &[timestamp(1, 0x9000), fence(0x8000)])?;
    rig.drive()?;
    // When: channel reset cancels old work; the same channel is configured for a new command.
    let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    rig.dma.write(reset, &1_u32.to_le_bytes())?;
    rig.drive()?;
    let mut busy = [0; 4];
    rig.dma.read(reset, &mut busy)?;
    assert_eq!(busy, [0; 4]);
    timer.0.store(81, Ordering::Relaxed);
    rig.dma.memory.fault.set(false);
    let mut rig = Rig::assemble(rig.dma, rig.memory, 1)?;
    rig.submit(0, &[timestamp(2, 0x9010), fence(0x8010)])?;
    rig.drive()?;
    // Then: reset did not publish the old sample, reuse it, or reset/disconnect the timer.
    assert_eq!(rig.word(0x9000)?, 0);
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.word(0x9010)?, 81);
    assert_eq!(rig.word(0x8010)?, 0xfeed);
    Ok(())
}

#[test]
fn context_drain_detects_parked_timestamp_and_abort_discards_it() -> TestResult {
    // Given: Fence and Timestamp both retain their target context while parked.
    let context = ContextId::new(7);
    for command in [
        Command::timestamp(
            1,
            TimestampCommand {
                address: 0x9000,
                context: context.raw(),
            },
        ),
        Command::fence(
            1,
            WordWrite {
                target: pdma_model_contract::dma::WordTarget {
                    context: context.raw(),
                    ..crate::command_operations::support::target(0x9000)
                },
                value: 17,
            },
        ),
    ] {
        let mut rig = rig(0x9000)?;
        rig.dma = rig
            .dma
            .with_timer(Arc::new(PlatformTimer(AtomicU64::new(17))));
        rig.submit(0, &[command, fence(0x8000)])?;
        rig.drive()?;
        // When: Drain cannot complete, and software chooses Abort instead.
        rig.dma.drain_context(context, &[ChannelId::new(0)])?;
        assert_eq!(
            rig.dma.context_status(context),
            Err(MaintenanceError::ChannelStopped(ChannelId::new(0)))
        );
        rig.dma.abort_context(context, &[ChannelId::new(0)])?;
        rig.drive()?;
        assert!(rig.dma.context_status(context)?);
        rig.dma.memory.fault.set(false);
        resume(&mut rig)?;
        rig.drive()?;
        // Then: no late sample/value is published; unrelated-context successor work can complete.
        assert_eq!(rig.word(0x9000)?, 0);
        assert_eq!(rig.word(0x8000)?, 0xfeed);
        assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    }
    Ok(())
}
