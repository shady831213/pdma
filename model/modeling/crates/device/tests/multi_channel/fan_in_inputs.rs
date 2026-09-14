use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Arbitrate, BranchError, Operation, ReadyStage, StepOutcome};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::{ChannelBlock, ChannelId, CommandRegisters};
use pdma_model_device::{Device, Error, Fetch, RoundRobin};

fn device() -> TestResult<Device<SharedMemory>> {
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x3000,
            MemoryAlignment::ONE,
        )?])?
        .try_install(&memory)?;
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
        let task = match Operation::new(()).step(&mut combined, &mut device) {
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
    let result = Operation::new(()).step(&mut combined, &mut device);
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
