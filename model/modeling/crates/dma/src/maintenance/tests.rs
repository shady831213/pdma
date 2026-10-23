use crate::{Dma, MaintenanceError};
use pdma_model_contract::dma::{ChannelId, ContextId};
use pdma_model_contract::register::{ReadWordValues, SwitchValues, WordValues};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn peer_reset_does_not_interrupt_an_unrelated_drain() -> TestResult {
    // Given: channel zero has an outstanding frontier, outside channel one's reset scope.
    let mut dma = Dma::new((), 2)?;
    let channel = dma.channels.first_mut().ok_or("channel")?;
    channel.registers.values.config.enable = SwitchValues::new(1);
    channel.command.registers.values.producer = WordValues::new(1);
    let target = ContextId::new(7);
    dma.drain_context(target, &[ChannelId::new(0)])?;
    // When: software resets only the peer channel through administration MMIO.
    let reset = ChannelId::new(1).admin_offset(2).ok_or("reset layout")?;
    dma.write(reset, &1_u32.to_le_bytes())?;
    // Then: the original frontier remains pending without failed channels.
    assert_eq!(dma.context_status(target), Ok(false));
    assert_eq!(dma.failed_context_channels(target).count(), 0);
    Ok(())
}

#[test]
fn duplicate_channel_frontiers_complete_when_the_owner_consumes_the_snapshot() -> TestResult {
    // Given: the same channel appears twice in a captured producer frontier.
    let mut dma = Dma::new((), 1)?;
    let channel = dma.channels.first_mut().ok_or("channel")?;
    channel.registers.values.config.enable = SwitchValues::new(1);
    channel.command.registers.values.capacity = WordValues::new(2);
    channel.command.registers.values.producer = WordValues::new(1);
    let id = ChannelId::new(0);
    let context = ContextId::new(7);
    dma.drain_context(context, &[id, id])?;
    assert_eq!(dma.context_status(context), Ok(false));
    // When: the owner consumes the captured entry.
    dma.channels
        .first_mut()
        .ok_or("channel")?
        .command
        .consume()
        .map_err(|error| format!("{error:?}"))?;
    dma.observe_channel(id);
    // Then: both frontier records are satisfied.
    assert_eq!(dma.context_status(context), Ok(true));
    Ok(())
}

#[test]
fn abort_preserves_parked_matching_work_outside_its_channel_scope() -> TestResult {
    // Given: a matching parked command belongs to a channel outside the operation scope.
    let (mut dma, command) = parked_peer()?;
    // When: Abort targets only the other channel.
    dma.abort_context(ContextId::new(7), &[ChannelId::new(0)])?;
    // Then: the operation completes without retiring the peer's parked command.
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    let peer = dma.channels.get(1).ok_or("channel")?;
    assert_eq!(peer.accepted_command(), Some(&command));
    assert!(peer.registers.values.recovery.fault_status.faulted());
    Ok(())
}

fn parked_peer() -> Result<(Dma<()>, pdma_model_contract::dma::Command), Box<dyn std::error::Error>>
{
    let mut dma = Dma::new((), 2)?;
    let command = pdma_model_contract::dma::Command::copy(
        17,
        pdma_model_contract::dma::CopyCommand {
            copy_length: 4,
            sg_context: 0,
            source: pdma_model_contract::dma::Endpoint {
                context: 7,
                ..pdma_model_contract::dma::Endpoint::linear(0x1000, 0)
            },
            destination: pdma_model_contract::dma::Endpoint {
                context: 7,
                ..pdma_model_contract::dma::Endpoint::linear(0x2000, 0)
            },
        },
    );
    let peer = dma.channels.get_mut(1).ok_or("channel")?;
    peer.work = Some(crate::channel::Accepted {
        fill_completed: 0,
        timestamp: None,
        command,
        unread: None,
        payload: None,
        progress: None,
    });
    peer.suspend();
    Ok((dma, command))
}

#[test]
fn abort_retires_resumed_work_before_it_is_regranted() -> TestResult {
    // Given: software resumed parked work but arbitration has not regranted it.
    let (mut dma, _) = parked_peer()?;
    let base = ChannelId::new(1).register_offset(2).ok_or("layout")?;
    dma.write(
        base + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
            + pdma_model_contract::dma::ChannelRegisters::RECOVERY.offset()
            + pdma_model_contract::dma::ChannelRecovery::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    // When: Abort targets the resumed command before its next grant.
    dma.abort_context(ContextId::new(7), &[ChannelId::new(1)])?;
    // Then: retained work is discarded without executing or waiting for another resume.
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    assert!(
        dma.channels
            .get(1)
            .ok_or("channel")?
            .accepted_command()
            .is_none()
    );
    Ok(())
}

#[test]
fn pending_maintenance_collects_simultaneous_failures_and_freezes_them() -> TestResult {
    // Given: three channels with outstanding frontiers for one context.
    let mut dma = Dma::new((), 3)?;
    for channel in &mut dma.channels {
        channel.registers.values.config.enable = SwitchValues::new(1);
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
        channel.registers.values.config.enable = SwitchValues::new(1);
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
    channel.registers.values.config.enable = SwitchValues::new(1);
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
