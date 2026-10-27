use super::support::{TestResult, configure, finish, submit, word};
use hardware_interface_memory::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use hardware_interface_pipeline::{Arbitration, Operation, StepOutcome};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, CommandRegisters};
use pdma_model_dma::{Candidate, CommandRequest, Dma, Error, RoundRobin};

fn device() -> TestResult<Dma<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x3000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory, 3)?;
    for index in 0..3 {
        configure(&mut device, index)?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    Ok(device)
}

fn consumer(device: &mut Dma<SharedMemory>, index: usize) -> TestResult<u32> {
    word(
        device,
        ChannelId::new(index).register_offset(3).ok_or("offset")?
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::CONSUMER.offset(),
    )
}

#[test]
fn repeated_candidate_production_leaves_every_descriptor_unaccepted() -> TestResult {
    // Given: every channel has a queued command.
    let mut device = device()?;
    let mut inputs = [0, 1, 2].map(|index| Candidate::new(ChannelId::new(index)));
    // When: the arbiter repeatedly inspects eligibility without granting an input.
    for _ in 0..3 {
        for (index, input) in inputs.iter_mut().enumerate() {
            let StepOutcome::Complete(Ok(Some(request))) =
                Operation::new(()).step(input, &mut device)?
            else {
                return Err("missing candidate".into());
            };
            assert_eq!(request.channel(), ChannelId::new(index));
            assert!(matches!(request, CommandRequest::Fetch(_)));
        }
    }
    // Then: all commands remain queued.
    for index in 0..3 {
        assert_eq!(consumer(&mut device, index)?, 0);
    }
    Ok(())
}

#[test]
fn round_robin_selects_only_its_explicit_candidate_values() -> TestResult {
    // Given: a channel subset whose input order differs from hardware indices.
    let mut device = device()?;
    let mut values = Vec::new();
    for index in [2, 0] {
        let mut candidate = Candidate::new(ChannelId::new(index));
        values.push(Some(finish(
            &mut candidate,
            &mut Operation::new(()),
            &mut device,
        )??));
    }
    let mut policy = RoundRobin::default();
    // When: two rounds select from the completed candidate values.
    for index in [2, 0] {
        let selected = policy.select(&values)?.ok_or("missing candidate")?;
        assert_eq!(
            values
                .get(selected)
                .copied()
                .flatten()
                .flatten()
                .map(CommandRequest::channel),
            Some(ChannelId::new(index))
        );
    }
    // Then: selection neither includes the excluded channel nor consumes descriptors.
    for index in 0..3 {
        assert_eq!(consumer(&mut device, index)?, 0);
    }
    Ok(())
}

#[test]
fn invalid_candidate_target_reports_an_error_without_consuming_valid_inputs() -> TestResult {
    // Given: a candidate stage bound outside the device's channel population.
    let mut device = device()?;
    let mut candidate = Candidate::new(ChannelId::new(3));
    // When: the invalid channel is fetched.
    let mut operation = Operation::new(());
    let result = finish(&mut candidate, &mut operation, &mut device)?;
    // Then: configuration misuse is reported rather than waiting forever or touching another channel.
    assert!(matches!(result, Err(Error::Unmapped)));
    for index in 0..3 {
        assert_eq!(consumer(&mut device, index)?, 0);
    }
    Ok(())
}
