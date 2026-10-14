use crate::{
    RESET,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{Operation, RunError, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, ResponseRegisters,
};
use pdma_model_dma::{Dma, Error, Fetch, ResetControl, Service, WriteCopy};

#[test]
fn reset_retires_channel_work_while_an_old_token_is_retained() -> TestResult {
    // Given: a real descriptor accepted into Channel state and a separately retained work token.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(0x1300.into(), MemoryAlignment::ONE, &[0xaa; 4])?;
    submit(&mut device, &memory, command())?;
    let request = device.command_request(ChannelId::new(0))?;
    let StepOutcome::Complete(Ok(Some(work))) =
        Operation::new(request).step(&mut Fetch, &mut device)?
    else {
        return Err("command not accepted".into());
    };
    device.write(RESET, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, RESET)?, 1);
    let mut service = device.service();
    let mut cleanup = Operation::new(());
    // When: service advances concrete hardware cleanup with the old token still live.
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = cleanup.step(&mut service, &mut device)? {
            result?;
            break;
        }
    }
    // Then: reset completes, and neither a stale payload stage nor publication can cause effects.
    assert_eq!(read_word(&mut device, RESET)?, 0);
    assert!(matches!(
        Operation::new(Some(work)).step(&mut WriteCopy, &mut device)?,
        StepOutcome::Complete(Err(Error::Reset))
    ));
    let mut responses = device.responses();
    let mut stale = Operation::new(Some(work));
    let mut result = None;
    for _ in 0..32 {
        if let StepOutcome::Complete(value) = stale.step(&mut responses, &mut device)? {
            result = Some(value);
            break;
        }
    }
    assert_eq!(result, Some(Err(Error::Reset)));
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [0xaa; 4]
    );
    Ok(())
}

#[test]
fn stale_work_cannot_retire_or_write_a_newer_accepted_command() -> TestResult {
    // Given: a completed old service invocation and a reset channel configured for new work.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let request = device.command_request(ChannelId::new(0))?;
    let StepOutcome::Complete(Ok(Some(old_work))) =
        Operation::new(request).step(&mut Fetch, &mut device)?
    else {
        return Err("command not accepted".into());
    };
    device.write(RESET, &1_u32.to_le_bytes())?;
    let mut service = device.service();
    let mut old = <Service as Stage<Dma<SharedMemory>>>::start(());
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = service.step(&mut device, &mut old)? {
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
    let request = device.command_request(ChannelId::new(0))?;
    assert!(matches!(
        Operation::new(request).step(&mut Fetch, &mut device)?,
        StepOutcome::Complete(Ok(Some(_)))
    ));
    // When: stale stage and already completed service states are driven after new acceptance.
    assert!(matches!(
        Operation::new(Some(old_work)).step(&mut WriteCopy, &mut device)?,
        StepOutcome::Complete(Err(Error::Reset))
    ));
    assert_eq!(
        service.step(&mut device, &mut old),
        Err(RunError::Exhausted)
    );
    crate::support::run(&mut device)?;
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
