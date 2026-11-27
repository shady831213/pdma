use super::support::{EVENT_PRODUCER, Rig, TestResult, target};
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelId, Command, CommandHeader, ContextId, DmaAdmin, DmaDiscovery, Opcode, WaitCommand,
    WaitCommandComparison, WordTarget,
};

const fn wait() -> Command {
    Command::Wait(WaitCommand {
        header: CommandHeader {
            opcode: Opcode::Wait,
        },
        id: 1,
        target: WordTarget {
            context: 1,
            ..target(0x8000)
        },
        value: 1,
        comparison: WaitCommandComparison::Equal,
    })
}

#[test]
fn drain_waits_for_ordinary_dependency_without_channel_stopped() -> TestResult {
    // Given: an unsatisfied dependency in a payload-only context.
    let mut rig = Rig::new(1, 0)?;
    rig.submit(0, &[wait()])?;
    rig.drive()?;
    let context = ContextId::new(1);
    // When: Drain snapshots the wait and software later satisfies it.
    rig.dma.drain_context(context, &[ChannelId::new(0)])?;
    rig.drive()?;
    assert!(!rig.dma.context_status(context)?);
    rig.memory.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &1_u64.to_le_bytes(),
    )?;
    rig.drive()?;
    // Then: Drain completes through actual retirement without fault recovery.
    assert!(rig.dma.context_status(context)?);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn reset_cancels_ordinary_wait_without_resume() -> TestResult {
    // Given: retained dependency work with no fault episode.
    let mut rig = Rig::new(1, 0)?;
    rig.submit(0, &[wait()])?;
    rig.drive()?;
    // When: software resets its channel while the dependency remains false.
    let reset = DmaDiscovery::EXTENT + DmaAdmin::CHANNELS_OFFSET;
    rig.dma.write(reset, &1_u32.to_le_bytes())?;
    rig.drive()?;
    // Then: cleanup completes without satisfying the dependency or emitting an event.
    let mut bytes = [0; 4];
    rig.dma.read(reset, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    assert_eq!(rig.word(0x8000)?, 0);
    assert_eq!(rig.register(0, EVENT_PRODUCER)?, 0);
    Ok(())
}

#[test]
fn drain_of_a_disabled_yielded_wait_reports_the_owner() -> TestResult {
    use pdma_model_contract::dma::{ChannelBlock, ChannelConfiguration, ChannelRegisters};
    use pdma_model_dma::{CommandRequest, MaintenanceError};

    let mut rig = Rig::new(1, 0)?;
    let id = ChannelId::new(0);
    let context = ContextId::new(1);
    rig.submit(0, &[wait()])?;
    for _ in 0..512 {
        rig.step()?;
        if matches!(
            rig.dma.command_request(id)?,
            Some(CommandRequest::Resume(_))
        ) {
            break;
        }
    }
    assert!(matches!(
        rig.dma.command_request(id)?,
        Some(CommandRequest::Resume(_))
    ));
    let enable = id.register_offset(1).ok_or("channel")?
        + ChannelBlock::CONTROL.offset()
        + ChannelRegisters::CONFIG.offset()
        + ChannelConfiguration::ENABLE.offset();
    rig.dma.write(enable, &0_u32.to_le_bytes())?;
    rig.dma.drain_context(context, &[id])?;
    rig.drive()?;
    assert_eq!(
        rig.dma.context_status(context),
        Err(MaintenanceError::ChannelStopped(id))
    );
    rig.dma.abort_context(context, &[id])?;
    rig.drive()?;
    assert!(rig.dma.context_status(context)?);
    Ok(())
}
