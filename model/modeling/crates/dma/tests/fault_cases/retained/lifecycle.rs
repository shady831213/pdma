use super::*;
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::ResetControl;

#[test]
fn reset_at_suspension_handoff_drops_enclosing_state_before_acknowledgement() -> TestResult {
    // Given: the execution turn has yielded but its continuation has not reached the bank.
    let mut dma = setup()?.dma;
    let command = streams(&dma)?;
    dma.memory.missing.set(Some((0x1343, true)));
    submit(&mut dma, command, 0)?;
    let drops = Rc::new(Cell::new(0));
    let mut engine = dma.command_loop_with(Enclosing::new(
        &dma,
        Rc::clone(&drops),
        Rc::new(Cell::new(0)),
    ));
    let mut operation = Operation::new(());
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
        if fault_status(&mut dma)? == 1 {
            break;
        }
    }
    assert_eq!(fault_status(&mut dma)?, 1);
    assert_eq!(drops.get(), 0);
    let reset = ChannelId::new(0).admin_offset(1).ok_or("reset")?;
    // When: reset arrives at the transfer boundary before the explicit park stage runs.
    dma.write(reset, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut dma, ());
    assert_eq!(read_word(&mut dma, reset)?, 1);
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
        if read_word(&mut dma, reset)? == 0 {
            break;
        }
    }
    // Then: hardware acknowledges only after dropping the exact suspended child state.
    assert_eq!(read_word(&mut dma, reset)?, 0);
    assert_eq!(drops.get(), 1);
    assert_eq!(fault_status(&mut dma)?, 0);
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    assert_eq!(
        dma.memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        *b"c-----"
    );
    Ok(())
}

#[test]
fn abandoning_parked_simulation_state_drops_it_without_acknowledging_hardware_reset() -> TestResult
{
    // Given: an owned non-Clone continuation is suspended in the channel bank.
    let mut dma = setup()?.dma;
    let command = streams(&dma)?;
    dma.memory.missing.set(Some((0x1343, true)));
    submit(&mut dma, command, 0)?;
    let drops = Rc::new(Cell::new(0));
    let mut engine = dma.command_loop_with(Enclosing::new(
        &dma,
        Rc::clone(&drops),
        Rc::new(Cell::new(0)),
    ));
    let mut operation = Operation::new(());
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
    }
    assert_eq!(fault_status(&mut dma)?, 1);
    assert_eq!(drops.get(), 0);
    // When: the simulation operation is cancelled independently of hardware lifecycle.
    operation.reset();
    let reset = ChannelId::new(0).admin_offset(1).ok_or("reset")?;
    dma.write(reset, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut dma, ());
    // Then: Rust state drops once; cancellation does not pretend hardware cleanup completed.
    assert_eq!(drops.get(), 1);
    assert_eq!(read_word(&mut dma, reset)?, 1);
    assert!(matches!(
        operation.step(&mut engine, &mut dma),
        Err(AlreadyComplete)
    ));
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    Ok(())
}
