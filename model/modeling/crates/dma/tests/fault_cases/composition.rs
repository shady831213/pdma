use super::*;
use hardware_interface_pipeline::{Stage, StepResult, Then};
use pdma_model_dma::{CheckRanges, ExecutionContext, Payload, PrepareChunk, ReadCopy, WriteCopy};

pub(super) struct InvertPayload;

impl Stage<ExecutionContext<'_, Paged>> for InvertPayload {
    type Input = pdma_model_dma::Flow<Payload>;
    type Output = pdma_model_dma::Flow<Payload>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, Paged>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(mut work) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        if let Some(data) = work.data_mut() {
            match data.bytes_mut() {
                Ok(bytes) => bytes.iter_mut().for_each(|byte| *byte = !*byte),
                Err(error) => return Ok(StepOutcome::Complete(Ok(work.stop(error)))),
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
        PrepareChunk::default(),
        Then::new(ReadCopy::default(), InvertPayload),
    );
    let execution = chunk;
    let pipeline = Then::new(
        execution,
        Then::new(
            WriteCopy::default(),
            Then::new(pdma_model_dma::CompletionInput, device.responses()),
        ),
    );
    let mut engine = device.hardware_with(pipeline);
    let mut operation = Operation::new(());
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut engine, &mut device)? {
            result?;
            if fault_status(&mut device)? == 1 {
                break;
            }
            operation = Operation::new(());
        }
    }
    // When: a repeated fault and eventual repair resume that same composed operation.
    assert_eq!(fault_status(&mut device)?, 1);
    assert!(operation.is_complete());
    operation = Operation::new(());
    resume(&mut device, 1)?;
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut engine, &mut device)? {
            result?;
            if fault_status(&mut device)? == 1 {
                break;
            }
            operation = Operation::new(());
        }
    }
    assert_eq!(fault_status(&mut device)?, 1);
    assert!(operation.is_complete());
    operation = Operation::new(());
    device.memory.missing.set(None);
    if replace_service {
        let chunk = Then::new(
            PrepareChunk::default(),
            Then::new(ReadCopy::default(), InvertPayload),
        );
        engine = device.hardware_with(Then::new(
            chunk,
            Then::new(
                WriteCopy::default(),
                Then::new(pdma_model_dma::CompletionInput, device.responses()),
            ),
        ));
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
        if let StepOutcome::Complete(result) = operation.step(&mut engine, &mut device)? {
            result?;
            operation = Operation::new(());
        }
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
    let StepOutcome::Complete(Ok(empty)) =
        Operation::new(None).step(&mut pdma_model_dma::Fetch, &mut device.dma)?
    else {
        return Err("empty Fetch did not complete".into());
    };
    let mut state = Some(empty);
    let mut validation = CheckRanges::default();
    let mut complete = false;
    for _ in 0..16 {
        if let StepOutcome::Complete(result) =
            validation.step(&mut device.dma.execution_context(), &mut state)?
        {
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
