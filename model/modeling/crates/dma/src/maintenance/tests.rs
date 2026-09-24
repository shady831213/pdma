use crate::{Dma, MaintenanceError};
use pdma_model_contract::dma::{ChannelId, ContextId};
use pdma_model_contract::register::{ReadWordValues, SwitchValues, WordValues};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn pending_maintenance_collects_simultaneous_failures_and_freezes_them() -> TestResult {
    // Given: three channels with outstanding frontiers for one context.
    let mut dma = Dma::new((), 3)?;
    for channel in &mut dma.channels {
        channel.registers.values.enable = SwitchValues::new(1);
        channel.command.registers.values.producer = WordValues::new(1);
    }
    let context = ContextId::new(7);
    dma.drain_context(
        context,
        &[ChannelId::new(0), ChannelId::new(1), ChannelId::new(2)],
    )?;
    assert_eq!(dma.context_status(context), Ok(false));
    // When: two channels stop before the next maintenance observation.
    dma.channels.get_mut(0).ok_or("channel")?.request_reset();
    dma.channels.get_mut(2).ok_or("channel")?.request_reset();
    dma.refresh_maintenance();
    // Then: both failures are retained, and later faults do not change the snapshot.
    assert_eq!(
        dma.failed_context_channels(context).collect::<Vec<_>>(),
        [ChannelId::new(0), ChannelId::new(2)]
    );
    assert_eq!(
        dma.context_status(context),
        Err(MaintenanceError::ChannelStopped(ChannelId::new(0)))
    );
    dma.channels.get_mut(1).ok_or("channel")?.request_reset();
    dma.channels.get_mut(0).ok_or("channel")?.finish_reset();
    dma.refresh_maintenance();
    assert_eq!(
        dma.failed_context_channels(context).collect::<Vec<_>>(),
        [ChannelId::new(0), ChannelId::new(2)]
    );
    assert_eq!(dma.maintenance_interlock(None), Ok(()));
    Ok(())
}

#[test]
fn wrapping_frontiers_are_fixed_and_passed_frontiers_stay_passed() -> TestResult {
    // Given
    let mut dma = Dma::new((), 2)?;
    for channel in &mut dma.channels {
        channel.registers.values.enable = SwitchValues::new(1);
        channel.command.registers.values.capacity = WordValues::new(4);
        channel.command.registers.values.producer = WordValues::new(1);
    }
    dma.channels
        .get_mut(0)
        .ok_or("channel")?
        .command
        .registers
        .values
        .consumer = ReadWordValues::new(u32::MAX - 1);
    dma.drain_context(ContextId::new(7), &[ChannelId::new(0), ChannelId::new(1)])?;
    // When
    for _ in 0..3 {
        dma.channels
            .get_mut(0)
            .ok_or("channel")?
            .command
            .consume()
            .map_err(|e| format!("{e:?}"))?;
        dma.refresh_maintenance();
    }
    // Then
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(false));
    assert!(
        dma.maintenance
            .as_ref()
            .ok_or("maintenance")?
            .frontiers
            .first()
            .ok_or("frontier")?
            .passed
    );
    let pointers = &mut dma
        .channels
        .get_mut(0)
        .ok_or("channel")?
        .command
        .registers
        .values;
    pointers.producer = WordValues::new(3);
    pointers.consumer = ReadWordValues::new(0);
    dma.channels
        .get_mut(1)
        .ok_or("channel")?
        .command
        .consume()
        .map_err(|e| format!("{e:?}"))?;
    dma.refresh_maintenance();
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    Ok(())
}

#[test]
fn new_queue_incarnation_cannot_satisfy_an_old_frontier() -> TestResult {
    // Given
    let mut dma = Dma::new((), 1)?;
    let channel = dma.channels.first_mut().ok_or("channel")?;
    channel.registers.values.enable = SwitchValues::new(1);
    channel.command.registers.values.producer = WordValues::new(1);
    dma.drain_context(ContextId::new(7), &[ChannelId::new(0)])?;
    // When
    let channel = dma.channels.first_mut().ok_or("channel")?;
    channel.request_reset();
    channel.finish_reset();
    dma.refresh_maintenance();
    // Then
    assert_eq!(
        dma.context_status(ContextId::new(7)),
        Err(MaintenanceError::ChannelStopped(ChannelId::new(0)))
    );
    assert_eq!(dma.maintenance_interlock(None), Ok(()));
    Ok(())
}
