use crate::{
    RESET,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{Operation, RunError, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, ResponseRegisters,
};
use pdma_model_dma::{Completion, Dma, Error, Payload, PrepareChunk, Publish, ReadCopy, WriteCopy};

fn payload(device: &mut Dma<SharedMemory>) -> TestResult<Payload> {
    let input = crate::support::accept(device, ChannelId::new(0))?;
    let mut pipeline = Then::new(PrepareChunk::default(), ReadCopy::default());
    let mut context = device.execution_context(ChannelId::new(0))?;
    let mut operation = Operation::new(Some(input));
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut pipeline, &mut context)? {
            return result?.ok_or_else(|| "missing payload".into());
        }
    }
    Err("payload watchdog".into())
}

enum Retained {
    Payload(Box<Payload>),
    Completion(Completion),
}

#[test]
fn reset_retires_channel_work_while_an_old_token_is_retained() -> TestResult {
    for publish in [false, true] {
        // Given: actual payload or response retained outside the accepted ledger.
        let (mut device, memory) = setup(0x1100, 2)?;
        submit(&mut device, &memory, command())?;
        let bytes = payload(&mut device)?;
        let retained = if publish {
            let StepOutcome::Complete(Ok(Some(completion))) = Operation::new(Some(bytes)).step(
                &mut WriteCopy::default(),
                &mut device.execution_context(ChannelId::new(0))?,
            )?
            else {
                return Err("missing completion".into());
            };
            Retained::Completion(completion)
        } else {
            Retained::Payload(Box::new(bytes))
        };
        memory.write(0x1300.into(), MemoryAlignment::ONE, &[0xaa; 4])?;
        device.write(RESET, &1_u32.to_le_bytes())?;
        // When: reset cleans nonresident work while the typed value remains retained.
        // Then: stale write and publication inputs cannot access memory or retire new work.
        assert_eq!(read_word(&mut device, RESET)?, 0);
        match retained {
            Retained::Payload(bytes) => assert!(matches!(
                Operation::new(Some(*bytes)).step(
                    &mut WriteCopy::default(),
                    &mut device.execution_context(ChannelId::new(0))?
                )?,
                StepOutcome::Complete(Err(Error::Reset))
            )),
            Retained::Completion(completion) => assert!(matches!(
                Operation::new(completion).step(
                    &mut Publish::new(ChannelId::new(0)),
                    &mut device.execution_context(ChannelId::new(0))?
                )?,
                StepOutcome::Complete(Err(Error::Reset))
            )),
        }
        assert_eq!(
            memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
            [0xaa; 4]
        );
    }
    Ok(())
}

#[test]
fn stale_work_cannot_retire_or_write_a_newer_accepted_command() -> TestResult {
    // Given: a completed old service invocation and a reset channel configured for new work.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let old_work = payload(&mut device)?;
    device.write(RESET, &1_u32.to_le_bytes())?;
    let mut service = device.hardware();
    let mut old = Operation::new(());
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = old.step(&mut service, &mut device)? {
            result?;
            break;
        }
    }
    assert_eq!(read_word(&mut device, RESET)?, 0);
    let base = ChannelBlock::ALIGNMENT;
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
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
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(base + offset, &value.to_le_bytes())?;
    }
    let descriptor = pdma_model_contract::dma::Command {
        id: 81,
        ..command()
    };
    memory.write(0x1200.into(), MemoryAlignment::ONE, &[1, 2, 3, 4])?;
    submit(&mut device, &memory, descriptor)?;
    let input = crate::support::accept(&mut device, ChannelId::new(0))?;
    // When: stale stage and already completed service states are driven after new acceptance.
    assert!(matches!(
        Operation::new(Some(old_work)).step(
            &mut WriteCopy::default(),
            &mut device.execution_context(ChannelId::new(0))?
        )?,
        StepOutcome::Complete(Err(Error::Reset))
    ));
    assert_eq!(
        old.step(&mut service, &mut device),
        Err(RunError::Exhausted)
    );
    let mut pipeline = crate::support::chunk_pipeline(&device);
    let mut context = device.execution_context(ChannelId::new(0))?;
    let mut current = Operation::new(Some(input));
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = current.step(&mut pipeline, &mut context)? {
            result?;
            break;
        }
    }
    assert!(current.is_complete());
    // Then: the new descriptor completes normally and retains its own identity and payload.
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [1, 2, 3, 4]
    );
    let raw: pdma_model_contract::dma::ResponseRaw =
        memory.read_typed(0x1100.into(), MemoryAlignment::ONE)?;
    assert_eq!(
        pdma_model_contract::dma::Response::from_bytes(raw.as_ref())?.id,
        descriptor.id
    );
    Ok(())
}
