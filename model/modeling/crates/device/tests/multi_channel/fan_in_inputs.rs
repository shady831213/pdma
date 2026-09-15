use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Arbitrate, BranchError, Operation, ReadyStage, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, Response, ResponseStatus,
};
use pdma_model_device::{Device, Error, Fetch, ResetControl, RoundRobin};

fn device() -> TestResult<Device<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x3000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Device::new(memory, 3)?;
    for index in 0..3 {
        configure(&mut device, index)?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    Ok(device)
}

fn consumer(device: &mut Device<SharedMemory>, index: usize) -> TestResult<u32> {
    word(
        device,
        ChannelId::new(index).register_offset().ok_or("offset")?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::CONSUMER.offset(),
    )
}

#[test]
fn probing_fetch_readiness_does_not_consume_any_command() -> TestResult {
    // Given: every channel has a queued command.
    let mut device = device()?;
    let inputs = [0, 1, 2].map(|index| Fetch::new(ChannelId::new(index)));
    // When: the arbiter repeatedly inspects eligibility without granting an input.
    for _ in 0..3 {
        for input in &inputs {
            assert!(input.ready(&device));
        }
    }
    // Then: all commands remain queued.
    for index in 0..3 {
        assert_eq!(consumer(&mut device, index)?, 0);
    }
    Ok(())
}

#[test]
fn generic_fan_in_selects_only_its_explicit_channel_inputs() -> TestResult {
    // Given: a channel subset whose input order differs from hardware indices.
    let mut device = device()?;
    let mut combined = Arbitrate::new(
        vec![Fetch::new(ChannelId::new(2)), Fetch::new(ChannelId::new(0))],
        RoundRobin::default(),
    );
    // When: two successive commands cross the generic fan-in.
    for index in [2, 0] {
        let mut operation = Operation::new(());
        assert!(matches!(
            operation.step(&mut combined, &mut device),
            StepOutcome::Progressed
        ));
        assert!(matches!(
            operation.step(&mut combined, &mut device),
            StepOutcome::Progressed
        ));
        let task = match operation.step(&mut combined, &mut device) {
            StepOutcome::Complete(Ok(task)) => task,
            other => return Err(format!("unexpected fetch result: {other:?}").into()),
        };
        assert_eq!(task.channel(), ChannelId::new(index));
        assert_eq!(task.value.id, u64::try_from(index)?);
        assert_eq!(consumer(&mut device, index)?, 1);
        if index == 2 {
            assert_eq!(consumer(&mut device, 0)?, 0);
        }
    }
    // Then: the excluded channel was never consumed, despite being ready.
    assert_eq!(consumer(&mut device, 1)?, 0);
    Ok(())
}

#[test]
fn invalid_fetch_target_reports_an_error_without_consuming_valid_inputs() -> TestResult {
    // Given: a custom graph with an input outside the device's channel population.
    let mut device = device()?;
    let mut combined = Arbitrate::new(vec![Fetch::new(ChannelId::new(3))], RoundRobin::default());
    // When: the invalid input is selected.
    let mut operation = Operation::new(());
    assert!(matches!(
        operation.step(&mut combined, &mut device),
        StepOutcome::Progressed
    ));
    let result = operation.step(&mut combined, &mut device);
    // Then: configuration misuse is reported rather than waiting forever or touching another channel.
    assert!(matches!(
        result,
        StepOutcome::Complete(Err(BranchError::Application(Error::Unmapped)))
    ));
    for index in 0..3 {
        assert_eq!(consumer(&mut device, index)?, 0);
    }
    Ok(())
}

#[test]
fn reset_or_disable_before_acceptance_rearbitrates_without_consuming_the_selection() -> TestResult {
    for (register, value) in [
        (ChannelRegisters::RESET.offset(), 1_u32),
        (ChannelRegisters::ENABLE.offset(), 0),
    ] {
        // Given: A is selected, but no command has been accepted.
        let mut device = device()?;
        let mut intake = device.intake();
        let mut operation = Operation::new(());
        assert!(matches!(
            operation.step(&mut intake, &mut device),
            StepOutcome::Progressed
        ));
        assert_eq!(consumer(&mut device, 0)?, 0);
        // When: software makes A ineligible before the acceptance step.
        device.write(
            ChannelId::new(0).register_offset().ok_or("offset")?
                + ChannelBlock::CONTROL.offset()
                + register,
            &value.to_le_bytes(),
        )?;
        let selected = (0..32)
            .find_map(|_| match operation.step(&mut intake, &mut device) {
                StepOutcome::Complete(result) => Some(result),
                _ => None,
            })
            .ok_or("peer did not progress")??;
        // Then: B is accepted without consuming A or C.
        assert_eq!(selected.channel(), ChannelId::new(1));
        assert_eq!(consumer(&mut device, 0)?, 0);
        assert_eq!(consumer(&mut device, 1)?, 1);
        assert_eq!(consumer(&mut device, 2)?, 0);
    }
    Ok(())
}

#[test]
fn reset_and_reconfigure_before_acceptance_uses_the_new_command_generation() -> TestResult {
    // Given: the graph has selected A without accepting its old command.
    let mut device = device()?;
    let mut hardware = super::support::single_command(&device);
    let mut operation = Operation::new(());
    assert!(matches!(
        operation.step(&mut hardware, &mut device),
        StepOutcome::Progressed
    ));
    assert_eq!(consumer(&mut device, 0)?, 0);
    // When: software resets A and publishes a replacement command before acceptance.
    device.write(
        ChannelId::new(0).register_offset().ok_or("offset")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let reset = ChannelId::new(0).register_offset().ok_or("offset")?
        + ChannelBlock::CONTROL.offset()
        + ChannelRegisters::RESET.offset();
    assert_eq!(word(&mut device, reset)?, 1);
    let _ = Operation::new(()).step(&mut ResetControl, &mut device);
    assert_eq!(word(&mut device, reset)?, 0);
    configure(&mut device, 0)?;
    submit(&mut device, 0, 99)?;
    (0..128)
        .find_map(|_| match operation.step(&mut hardware, &mut device) {
            StepOutcome::Complete(result) => Some(result),
            _ => None,
        })
        .ok_or("replacement command did not finish")??;
    // Then: the replacement task completes under its fresh generation, not as stale work.
    let raw = device.memory.read_typed::<[u8; Response::RECORD_SIZE]>(
        MemoryAddress::new(0x1100),
        MemoryAlignment::ONE,
    )?;
    let response = Response::from_bytes(&raw)?;
    assert_eq!(response.id, 99);
    assert_eq!(response.status, ResponseStatus::Success);
    assert_eq!(response.bytes_completed, 512);
    assert_eq!(consumer(&mut device, 0)?, 1);
    assert_eq!(consumer(&mut device, 1)?, 0);
    Ok(())
}
