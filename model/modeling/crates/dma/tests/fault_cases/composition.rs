use super::*;
use hardware_interface_pipeline::{RunError, Stage, StepResult, Then};
use pdma_model_dma::{CheckRanges, Error, NextChunk, ReadCopy, Work, WriteCopy};

struct InvertPayload;

impl Stage<Dma<Paged>> for InvertPayload {
    type Input = Option<Work>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Option<Work>>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        device: &mut Dma<Paged>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(work) = state.take() else {
            return Err(RunError::Exhausted);
        };
        if let Some(work) = work {
            match device.work_scratch_mut(&work) {
                Ok(Some(bytes)) => bytes.iter_mut().for_each(|byte| *byte = !*byte),
                Ok(None) => {}
                Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            }
        }
        Ok(StepOutcome::Complete(Ok(work)))
    }
}

#[test]
fn write_resume_preserves_exact_position_after_non_idempotent_transform() -> TestResult {
    transformed_resume(false)
}

#[test]
fn replacing_service_preserves_channel_owned_transformed_write() -> TestResult {
    transformed_resume(true)
}

fn transformed_resume(replace_service: bool) -> TestResult {
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
    let pipeline = Then::new(execution, device.responses());
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
    if replace_service {
        let chunk = Then::new(
            Then::new(Then::new(NextChunk, ReadCopy), InvertPayload),
            WriteCopy,
        );
        engine = device.service_with(Then::new(Then::new(CheckRanges, chunk), device.responses()));
        operation = hardware_interface_pipeline::HardwareDriver::new();
        device.memory.memory.write(
            0x1000.into(),
            MemoryAlignment::ONE,
            &Command::ZERO.to_bytes()?,
        )?;
    }
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
    // When: an empty standalone invocation has no accepted work.
    let mut operation = Operation::new(None);
    let mut complete = false;
    for _ in 0..16 {
        if let StepOutcome::Complete(result) = operation.step(&mut CheckRanges, &mut device.dma)? {
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
