use super::support::{COUNT, Fixture, Result, offset, wait};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, ContextId};
use pdma_model_dma::MaintenanceError;

#[test]
fn drain_waits_for_signal_condition_instead_of_failing_channel() -> Result {
    // Given: a retained wait referencing a context distinct from its rings.
    let mut fixture = Fixture::new()?;
    fixture.submit(0, &[wait(1, 9)])?;
    fixture.tick(128)?;
    let context = ContextId::new(1);
    // When: Drain captures this unfinished wait.
    fixture.dma.drain_context(context, &[ChannelId::new(0)])?;
    fixture.tick(128)?;
    assert_eq!(fixture.dma.context_status(context), Ok(false));
    fixture.store(9)?;
    fixture.tick(128)?;
    // Then: normal completion releases its context reference.
    assert_eq!(fixture.produced(0)?, 1);
    assert_eq!(fixture.dma.context_status(context), Ok(true));
    Ok(())
}

#[test]
fn abort_discards_wait_without_modifying_signal() -> Result {
    // Given: an unmet wait.
    let mut fixture = Fixture::new()?;
    fixture.submit(0, &[wait(1, 9)])?;
    fixture.tick(128)?;
    let context = ContextId::new(1);
    // When: Abort retires the waiting target.
    fixture.dma.abort_context(context, &[ChannelId::new(0)])?;
    fixture.tick(128)?;
    // Then: the target is drained without a response or signal mutation.
    assert_eq!(fixture.dma.context_status(context), Ok(true));
    assert_eq!(fixture.produced(0)?, 0);
    assert_eq!(fixture.value()?, 0);
    Ok(())
}

#[test]
fn reset_can_finish_while_interrupting_drain_of_wait() -> Result {
    // Given: Drain blocked on an unmet signal.
    let mut fixture = Fixture::new()?;
    fixture.submit(0, &[wait(1, 9)])?;
    fixture.tick(128)?;
    let context = ContextId::new(1);
    fixture.dma.drain_context(context, &[ChannelId::new(0)])?;
    // When: the owner requests reset and hardware drains resident stages.
    let reset = ChannelId::new(0)
        .admin_offset(COUNT)
        .ok_or("reset offset")?;
    fixture.dma.write(reset, &1_u32.to_le_bytes())?;
    fixture.tick(128)?;
    // Then: failed Drain does not prevent successful reset cleanup.
    assert_eq!(
        fixture.dma.context_status(context),
        Err(MaintenanceError::ChannelStopped(ChannelId::new(0)))
    );
    let mut bytes = [0; 4];
    fixture.dma.read(reset, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    assert_eq!(fixture.value()?, 0);
    Ok(())
}

#[test]
fn drain_reports_disabled_wait_as_stopped() -> Result {
    let mut fixture = Fixture::new()?;
    fixture.submit(0, &[wait(1, 9)])?;
    fixture.tick(128)?;
    fixture.dma.write(
        offset(0)? + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
        &0_u32.to_le_bytes(),
    )?;
    let context = ContextId::new(1);
    fixture.dma.drain_context(context, &[ChannelId::new(0)])?;
    assert_eq!(
        fixture.dma.context_status(context),
        Err(MaintenanceError::ChannelStopped(ChannelId::new(0)))
    );
    Ok(())
}
