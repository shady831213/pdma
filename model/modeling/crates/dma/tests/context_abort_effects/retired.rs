use super::*;

#[test]
fn abort_after_retired_copy_has_no_sg_payload_event_or_irq_effects()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a real observed SG copy has retired without an event.
    let Fixture {
        mut dma,
        memory,
        mut pipeline,
        mut operation,
        normal,
    } = retired_copy_fixture()?;
    let channel = ChannelId::new(0);
    let base = channel.register_offset(1).ok_or("channel layout")?;
    let events = base + ChannelBlock::EVENT.offset();
    let pdma_model_contract::dma::Command::Copy(copy) = normal else {
        return Err("copy fixture".into());
    };
    let target = Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 2,
        copy_length: copy.copy_length,
        sg_context: 7,
        source: copy.source,
        source_context: 7,
        destination: copy.destination,
        destination_context: 7,
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    memory.write(0x1600.into(), MemoryAlignment::ONE, &[2, 4, 6, 8])?;
    memory.write(0x1700.into(), MemoryAlignment::ONE, &[9; 4])?;
    memory.write(0x1000.into(), MemoryAlignment::ONE, &target.to_bytes()?)?;
    dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut before = [0; 4096];
    memory.read(0x1000.into(), MemoryAlignment::ONE, &mut before)?;
    let log_start = dma.memory.accesses.borrow().len();
    let mut producer_before = [0; 4];
    dma.read(
        events + EventRegisters::PRODUCER.offset(),
        &mut producer_before,
    )?;
    assert_eq!(u32::from_le_bytes(producer_before), 0);
    // When: Abort arrives before the queued target is accepted.
    dma.abort_context(ContextId::new(7), &[channel])?;
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(false));
    for _ in 0..64 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma)?,
            StepOutcome::InProgress
        ));
        if dma.context_status(ContextId::new(7)) == Ok(true) {
            break;
        }
    }
    // Then: only command-ring reads occur; discarded work publishes nothing and leaves no pending cleanup.
    assert_eq!(dma.context_status(ContextId::new(7)), Ok(true));
    let accesses = dma.memory.accesses.borrow();
    let new_accesses = accesses.get(log_start..).ok_or("access log bounds")?;
    assert!(!new_accesses.is_empty());
    assert!(
        new_accesses
            .iter()
            .all(|access| *access == Access::Read(0x1000, Command::RECORD_SIZE)),
        "unexpected Abort effects: {new_accesses:?}"
    );
    drop(accesses);
    let mut after = [0; 4096];
    memory.read(0x1000.into(), MemoryAlignment::ONE, &mut after)?;
    assert_eq!(after, before);
    let mut producer_after = [0; 4];
    dma.read(
        events + EventRegisters::PRODUCER.offset(),
        &mut producer_after,
    )?;
    assert_eq!(producer_after, producer_before);
    let mut consumer = [0; 4];
    dma.read(base + CommandRegisters::CONSUMER.offset(), &mut consumer)?;
    assert_eq!(u32::from_le_bytes(consumer), 2);
    assert!(!dma.take_interrupt());
    Ok(())
}
