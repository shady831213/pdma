use super::*;
use pdma_model_contract::dma::{ChannelId, ContextId};

#[test]
fn parked_abort_finishes_while_an_accepted_peer_is_disabled() -> TestResult {
    // Given: one parked task and one accepted peer stalled by its enable register.
    let (mut dma, peer_response, _) = peers::pair()?;
    let mut command = streams(&dma)?;
    command.source.context = 7;
    dma.memory
        .memory
        .write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
    let peer = ChannelId::new(1).register_offset(2).ok_or("peer")?;
    let enable = peer + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset();
    let mut engine = dma.command_loop();
    let mut operation = Operation::new(());
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
        if read_word(&mut dma, peer + CommandRegisters::CONSUMER.offset())? == 1 {
            break;
        }
    }
    assert_eq!(fault_status(&mut dma)?, 1);
    assert_eq!(
        read_word(&mut dma, peer + CommandRegisters::CONSUMER.offset())?,
        1
    );
    dma.write(enable, &0_u32.to_le_bytes())?;
    // When: Abort targets the parked command independently of the disabled peer.
    let context = ContextId::new(7);
    dma.abort_context(context, &[ChannelId::new(0)])?;
    for _ in 0..128 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
        if dma.context_status(context) != Ok(false) {
            break;
        }
    }
    // Then: Abort retires its references without response; the peer resumes its original grant.
    assert_eq!(dma.context_status(context), Ok(true));
    assert_eq!(fault_status(&mut dma)?, 0);
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        read_word(
            &mut dma,
            peer_response + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    dma.write(enable, &1_u32.to_le_bytes())?;
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
    }
    assert_eq!(
        read_word(
            &mut dma,
            peer_response + ResponseRegisters::PRODUCER.offset()
        )?,
        1
    );
    assert_eq!(dma.memory.faults.get(), 1);
    Ok(())
}

#[test]
fn two_faulted_channels_resume_independently_through_the_shared_engine() -> TestResult {
    // Given: both channels read the same missing source page.
    let (mut dma, peer_response, peer_command) = peers::pair()?;
    dma.memory.missing.set(Some((0x1201, false)));
    let peer = ChannelId::new(1).register_offset(2).ok_or("peer")?;
    let peer_control = peer + ChannelBlock::CONTROL.offset();
    let mut engine = dma.command_loop();
    let mut operation = Operation::new(());
    for _ in 0..512 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
    }
    assert_eq!(fault_status(&mut dma)?, 1);
    assert_eq!(
        read_word(
            &mut dma,
            peer_control + ChannelRegisters::FAULT_STATUS.offset()
        )?,
        1
    );
    assert_eq!(dma.memory.faults.get(), 2);
    assert!(!dma.take_interrupt());
    // When: the backing is repaired, but software initially resumes only the peer.
    dma.memory.missing.set(None);
    dma.write(
        peer_control + ChannelRegisters::RESUME.offset(),
        &1_u32.to_le_bytes(),
    )?;
    for _ in 0..512 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
    }
    // Then: only the resumed channel publishes; a later resume completes the other once.
    assert_eq!(
        read_word(
            &mut dma,
            peer_response + ResponseRegisters::PRODUCER.offset()
        )?,
        1
    );
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(fault_status(&mut dma)?, 1);
    let raw: ResponseRaw = dma
        .memory
        .memory
        .read_typed(0x1800.into(), MemoryAlignment::ONE)?;
    assert_eq!(Response::from_bytes(raw.as_ref())?.id, peer_command.id);
    resume(&mut dma, 1)?;
    for _ in 0..512 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
    }
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        1
    );
    assert_eq!(
        read_word(
            &mut dma,
            peer_response + ResponseRegisters::PRODUCER.offset()
        )?,
        1
    );
    assert_eq!(
        dma.memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        *b"cdefgh"
    );
    assert_eq!(dma.memory.faults.get(), 2);
    Ok(())
}
