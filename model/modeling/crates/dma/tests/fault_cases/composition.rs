use super::*;
use hardware_interface_pipeline::{AfterEach, RunError, Stage, StepResult, Then};
use pdma_model_dma::{
    ChannelContext, CheckRanges, Error, NextChunk, ReadCopy, Resettable, Transfer, UpdateIrq,
    WriteCopy,
};

struct InvertPayload;

impl Stage<ChannelContext<'_, Paged>> for InvertPayload {
    type Input = Transfer;
    type Output = Transfer;
    type Error = Error;
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut ChannelContext<'_, Paged>,
        state: &mut Self::State,
    ) -> StepResult<Transfer, Error> {
        let Some(mut transfer) = state.take() else {
            return Err(RunError::Exhausted);
        };
        for byte in &mut transfer.bytes {
            *byte = !*byte;
        }
        Ok(StepOutcome::Complete(Ok(transfer)))
    }
}

#[test]
fn write_resume_preserves_exact_position_after_non_idempotent_transform() -> TestResult {
    // Given: an inserted payload transform already ran when destination access faults.
    let mut device = setup()?.dma;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1343, true)));
    submit(&mut device, command, 0)?;
    let chunk = Then::new(
        Then::new(Then::new(NextChunk, ReadCopy), InvertPayload),
        WriteCopy,
    );
    let execution = Then::new(CheckRanges, chunk);
    let pipeline = Then::new(
        Resettable::new(AfterEach::new(execution, UpdateIrq)),
        device.responses(),
    );
    let mut engine = device.service_with(pipeline);
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut device)?,
            StepOutcome::InProgress
        ));
        if fault_status(&mut device)? == 1 {
            break;
        }
    }
    // When: a repeated fault and eventual repair resume that same composed operation.
    assert_eq!(fault_status(&mut device)?, 1);
    resume(&mut device, 1)?;
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut device)?,
            StepOutcome::InProgress
        ));
        if fault_status(&mut device)? == 1 {
            break;
        }
    }
    assert_eq!(fault_status(&mut device)?, 1);
    device.memory.missing.set(None);
    device
        .memory
        .memory
        .write(0x1240.into(), MemoryAlignment::ONE, b"12345")?;
    resume(&mut device, 1)?;
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut device)?,
            StepOutcome::InProgress
        ));
        if read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())? == 1 {
            break;
        }
    }
    // Then: cached transformed bytes commit exactly once, without re-reading or re-transforming.
    assert_eq!(
        device
            .memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        b"cdefgh".map(|byte| !byte)
    );
    assert_eq!(response(&device)?.bytes_completed(), command.copy_length);
    assert_eq!(
        read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
        1
    );
    Ok(())
}

#[test]
fn standalone_range_validation_cannot_consume_parked_work() -> TestResult {
    // Given: the scheduler owns a suspended SG task.
    let mut device = setup()?;
    let command = streams(&device)?;
    device.memory.missing.set(Some((0x1242, false)));
    submit(&mut device, command, 0)?;
    device.until(|dma| Ok(fault_status(dma)? == 1))?;
    // When: a caller independently validates an unrelated zero-length command.
    let mut operation = Operation::new(pdma_model_dma::CommandInput::Fresh(Command::ZERO));
    let mut complete = false;
    let mut context = device.channel_context(pdma_model_contract::dma::ChannelId::new(0))?;
    for _ in 0..16 {
        if let StepOutcome::Complete(result) = operation.step(&mut CheckRanges, &mut context)? {
            let _progress = result?;
            complete = true;
            break;
        }
    }
    // Then: the parked task remains owned and resumes with its original progress.
    assert!(complete);
    assert_eq!(fault_status(&mut device)?, 1);
    device.memory.missing.set(None);
    resume(&mut device, 1)?;
    device.until(|dma| Ok(read_word(dma, RESP + ResponseRegisters::PRODUCER.offset())? == 1))?;
    assert_eq!(
        (response(&device)?.id, response(&device)?.bytes_completed()),
        (command.id, command.copy_length)
    );
    Ok(())
}
