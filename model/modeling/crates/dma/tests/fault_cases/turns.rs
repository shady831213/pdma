use super::*;
use hardware_interface_pipeline::{AfterEach, Repeat, Then};
use pdma_model_dma::{
    CheckRanges, CommandTurn, NextChunk, ReadCopy, Resettable, RunCommand, UpdateIrq, WriteCopy,
};

#[test]
fn standalone_execution_turn_completes_successfully_on_payload_fault() -> TestResult {
    // Given: a real accepted SG command and a missing destination page.
    let mut dma = setup()?.dma;
    let command = streams(&dma)?;
    dma.memory.missing.set(Some((0x1343, true)));
    submit(&mut dma, command, 0)?;
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckRanges, Repeat::new(chunk));
    let pipeline = Then::new(
        dma.intake(),
        Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            dma.responses(),
        ),
    );
    let mut turn = RunCommand::new(pipeline);
    let mut operation = Operation::new(());
    // When: the execution boundary observes the payload fault without any arbiter.
    let mut terminal = false;
    for _ in 0..256 {
        if let RunOutcome::Complete(result) = operation.step(&mut turn, &mut dma)? {
            assert!(matches!(result?, CommandTurn::Suspended(_)));
            terminal = true;
            break;
        }
    }
    // Then: suspension is a successful terminal turn, with no response or IRQ.
    assert_eq!(fault_status(&mut dma)?, 1);
    assert!(
        terminal,
        "payload fault must terminalize the execution turn"
    );
    assert!(operation.is_complete());
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        0
    );
    assert!(!dma.take_interrupt());
    Ok(())
}
