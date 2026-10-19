use super::{composition::InvertPayload, *};
use hardware_interface_pipeline::Then;
use pdma_model_dma::{PrepareChunk, ReadCopy, WriteCopy};

#[test]
fn command_invocation_transforms_every_chunk_with_one_memory_access_per_step() -> TestResult {
    chunked_transform(false)
}

#[test]
fn early_write_resume_ends_old_invocation_without_repeating_read_or_transform() -> TestResult {
    chunked_transform(true)
}

fn chunked_transform(fault: bool) -> TestResult {
    // Given: a three-chunk command with a non-idempotent payload transform.
    let mut device = setup()?.dma;
    let command = Command {
        copy_length: 600,
        source: Endpoint::linear(0x1400, 0),
        destination: Endpoint::linear(0x1800, 0),
        ..crate::support::command()
    };
    device
        .memory
        .memory
        .write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; 600])?;
    submit(&mut device, command, 0)?;
    if fault {
        device.memory.missing.set(Some((0x1900, true)));
    }
    let chunks = Then::new(
        PrepareChunk::default(),
        Then::new(ReadCopy::default(), InvertPayload),
    );
    let mut service = device.hardware_with(Then::new(
        pdma_model_dma::CommandExecution::new(Then::new(chunks, WriteCopy::default())),
        Then::new(pdma_model_dma::CompletionInput, device.responses()),
    ));
    let mut operation = Operation::new(());
    let mut resumed = false;
    let mut released_accesses = None;
    // When: software repairs and resumes immediately after the failed write step.
    for _ in 0..512 {
        let before = device.memory.accesses.get();
        let outcome = operation.step(&mut service, &mut device)?;
        let after = device.memory.accesses.get();
        assert!(
            after - before <= 1,
            "one hardware tick made multiple memory calls"
        );
        if let Some(released) = released_accesses {
            assert_eq!(after, released, "released invocation retried payload work");
        }
        if fault_status(&mut device)? == 1 {
            assert!(!resumed);
            assert_eq!(
                read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
                0
            );
            device.memory.missing.set(None);
            device
                .memory
                .memory
                .write(0x1500.into(), MemoryAlignment::ONE, &[0xaa; 256])?;
            resume(&mut device, 1)?;
            resumed = true;
            released_accesses = Some(after);
        }
        if let StepOutcome::Complete(result) = outcome {
            result?;
            if released_accesses.take().is_some() {
                assert_eq!(
                    read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
                    0
                );
            }
            if read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())? == 1 {
                break;
            }
            operation = Operation::new(());
        }
    }
    // Then: every chunk is transformed once; a parked write reuses its successful read.
    assert!(operation.is_complete());
    assert_eq!(resumed, fault);
    let mut bytes = [0; 600];
    device
        .memory
        .memory
        .read(0x1800.into(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [!0x5a; 600]);
    assert_eq!(
        *device.memory.reads.borrow(),
        [0x1000, 0x1400, 0x1500, 0x1600]
    );
    assert_eq!(
        read_word(&mut device, CMD + CommandRegisters::CONSUMER.offset())?,
        1
    );
    assert_eq!(
        read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
        1
    );
    assert_eq!(
        (response(&device)?.id, response(&device)?.bytes_completed()),
        (command.id, 600)
    );
    Ok(())
}
