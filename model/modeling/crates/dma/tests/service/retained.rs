use crate::{
    RESET,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{Operation, RunError, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, EventRegisters,
};
use pdma_model_dma::{
    Completion, Dma, Error, Flow, Payload, PrepareChunk, ReadCopy, Retire, WriteCopy,
};

fn payload(device: &mut Dma<SharedMemory>) -> TestResult<Flow<Payload>> {
    let input = crate::support::accept(device, ChannelId::new(0))?;
    let mut pipeline = Then::new(PrepareChunk::default(), ReadCopy::default());
    let mut context = device.execution_context();
    let mut operation = Operation::new(input);
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut pipeline, &mut context)? {
            let flow = result?;
            flow.status()?;
            assert!(flow.data().is_some());
            return Ok(flow);
        }
    }
    Err("payload watchdog".into())
}

enum Retained {
    Payload(Box<Flow<Payload>>),
    Completion(Flow<Completion>),
}

#[test]
fn reset_waits_for_retained_data_to_reach_retirement_without_old_effects() -> TestResult {
    for completed in [false, true] {
        // Given: actual payload or completion owns an unfinished hardware transfer.
        let (mut device, memory) = setup(0x1100, 2)?;
        submit(&mut device, &memory, command())?;
        let bytes = payload(&mut device)?;
        let retained = if completed {
            let StepOutcome::Complete(Ok(completion)) = Operation::new(bytes)
                .step(&mut WriteCopy::default(), &mut device.execution_context())?
            else {
                return Err("missing completion".into());
            };
            Retained::Completion(completion)
        } else {
            Retained::Payload(Box::new(bytes))
        };
        memory.write(0x1300.into(), MemoryAlignment::ONE, &[0xaa; 4])?;
        device.write(RESET, &1_u32.to_le_bytes())?;
        // When: reset stops the retained transfer before another memory interaction.
        assert_eq!(read_word(&mut device, RESET)?, 1);
        let stopped = match retained {
            Retained::Payload(bytes) => {
                let StepOutcome::Complete(Ok(flow)) = Operation::new(*bytes)
                    .step(&mut WriteCopy::default(), &mut device.execution_context())?
                else {
                    return Err("write did not observe reset".into());
                };
                flow.finish()
            }
            Retained::Completion(completion) => completion,
        };
        assert_eq!(
            stopped.lifecycle(&mut device.execution_context()),
            Err(Error::Reset)
        );
        assert_eq!(read_word(&mut device, RESET)?, 1);
        let result = Operation::new(vec![(0, stopped)]).step(&mut Retire, &mut device)?;
        // Then: only explicit retirement acknowledges reset; neither stopped path writes bytes.
        assert_eq!(result, StepOutcome::Complete(Err(Error::Reset)));
        assert_eq!(read_word(&mut device, RESET)?, 0);
        assert_eq!(
            memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
            [0xaa; 4]
        );
    }
    Ok(())
}

#[test]
fn stale_payload_is_rejected_before_effects_and_recovers_after_reset() -> TestResult {
    // Given: a completed old service invocation and a reset channel configured for new work.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut old_payload = None;
    let finished = payload(&mut device)?.map(|bytes| {
        old_payload = Some(bytes);
    });
    let mut old = Operation::new(vec![(0, finished.finish())]);
    assert_eq!(
        old.step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Ok(()))
    );
    device.write(RESET, &1_u32.to_le_bytes())?;
    assert_eq!(read_word(&mut device, RESET)?, 0);
    reconfigure(&mut device)?;
    let descriptor = pdma_model_contract::dma::Command {
        id: 81,
        ..command()
    };
    memory.write(0x1200.into(), MemoryAlignment::ONE, &[1, 2, 3, 4])?;
    submit(&mut device, &memory, descriptor)?;
    let input = crate::support::accept(&mut device, ChannelId::new(0))?;
    let old_payload = old_payload.ok_or("missing old payload")?;
    // When: a miswired transform substitutes obsolete bytes into the new flow.
    let StepOutcome::Complete(Ok(rejected)) = Operation::new(input.map(|_| old_payload))
        .step(&mut WriteCopy::default(), &mut device.execution_context())?
    else {
        return Err("stale payload not rejected".into());
    };
    assert_eq!(rejected.status(), Err(Error::Reset));
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [0; 4]
    );
    assert_eq!(
        read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset()
        )?,
        1
    );
    assert!(device.command_request(ChannelId::new(0))?.is_none());
    device.write(RESET, &1_u32.to_le_bytes())?;
    assert_eq!(read_word(&mut device, RESET)?, 1);
    assert_eq!(
        Operation::new(vec![(0, rejected.finish())]).step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Err(Error::Reset))
    );
    assert_eq!(read_word(&mut device, RESET)?, 0);
    reconfigure(&mut device)?;
    submit(&mut device, &memory, descriptor)?;
    crate::support::run(&mut device)?;
    // Then: the new descriptor completes normally and retains its own identity and payload.
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [1, 2, 3, 4]
    );
    crate::support::assert_no_event(&mut device)?;
    Ok(())
}

#[test]
fn completed_old_retirement_cannot_affect_a_newer_accepted_command() -> TestResult {
    // Given: an exhausted old operation and a separately accepted current command.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut old = Operation::new(vec![(0, payload(&mut device)?.finish())]);
    assert_eq!(
        old.step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Ok(()))
    );
    device.write(RESET, &1_u32.to_le_bytes())?;
    reconfigure(&mut device)?;
    let descriptor = pdma_model_contract::dma::Command {
        id: 81,
        ..command()
    };
    memory.write(0x1200.into(), MemoryAlignment::ONE, &[1, 2, 3, 4])?;
    submit(&mut device, &memory, descriptor)?;
    let input = crate::support::accept(&mut device, ChannelId::new(0))?;
    // When: the old retirement is stepped while the current command remains resident.
    assert_eq!(old.step(&mut Retire, &mut device), Err(RunError::Exhausted));
    let mut pipeline = crate::support::chunk_pipeline();
    let mut output = None;
    {
        let mut context = device.execution_context();
        let mut operation = Operation::new(input);
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(&mut pipeline, &mut context)? {
                output = Some(result?);
                break;
            }
        }
    }
    assert_eq!(
        Operation::new(vec![(0, output.ok_or("current command completion")?)])
            .step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Ok(()))
    );
    // Then: the current command completes with its own identity and bytes.
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [1, 2, 3, 4]
    );
    crate::support::assert_no_event(&mut device)?;
    Ok(())
}

fn reconfigure(device: &mut Dma<SharedMemory>) -> TestResult {
    let base = ChannelBlock::ALIGNMENT;
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
            0x1100,
        ),
    ] {
        device.write(base + offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(base + offset, &value.to_le_bytes())?;
    }
    Ok(())
}
