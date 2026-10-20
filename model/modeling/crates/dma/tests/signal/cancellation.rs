use super::support::{COUNT, Fixture, Result, signal, wait};
use pdma_model_contract::dma::{ChannelId, ContextId, SignalOperation};

#[test]
fn reset_at_each_pipeline_boundary_never_replays_an_atomic_update() -> Result {
    for ticks in 0..64 {
        // Given: an atomic increment at each possible early pipeline boundary.
        let mut fixture = Fixture::new()?;
        fixture.store(20)?;
        fixture.submit(0, &[signal(1, SignalOperation::Add, 7)])?;
        fixture.tick(ticks)?;
        let at_reset = fixture.value()?;
        // When: reset requests cleanup before hardware proceeds further.
        let reset = ChannelId::new(0)
            .admin_offset(COUNT)
            .ok_or("reset offset")?;
        fixture.dma.write(reset, &1_u32.to_le_bytes())?;
        fixture.tick(128)?;
        // Then: reset completes and never introduces another update.
        let mut bytes = [0; 4];
        fixture.dma.read(reset, &mut bytes)?;
        assert_eq!(u32::from_le_bytes(bytes), 0, "tick {ticks}");
        assert_eq!(fixture.value()?, at_reset, "tick {ticks}");
        assert!(matches!(at_reset, 20 | 27), "tick {ticks}");
    }
    Ok(())
}

#[test]
fn abort_at_each_wait_boundary_discards_without_late_response() -> Result {
    for ticks in 0..64 {
        // Given: an unmet Wait before or after it relinquishes execution.
        let mut fixture = Fixture::new()?;
        fixture.submit(0, &[wait(1, 9)])?;
        fixture.tick(ticks)?;
        let context = ContextId::new(1);
        // When: Abort captures it and the condition becomes true only after cleanup.
        fixture.dma.abort_context(context, &[ChannelId::new(0)])?;
        fixture.tick(256)?;
        assert_eq!(
            fixture.dma.context_status(context),
            Ok(true),
            "tick {ticks}"
        );
        fixture.store(9)?;
        fixture.tick(128)?;
        // Then: no late completion or signal modification escapes the canceled work.
        assert_eq!(fixture.produced(0)?, 0, "tick {ticks}");
        assert_eq!(fixture.value()?, 9, "tick {ticks}");
    }
    Ok(())
}
