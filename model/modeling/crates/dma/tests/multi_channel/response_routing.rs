use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, ResponseRegisters};
use pdma_model_dma::{
    CheckRanges, Completion, CompletionInput, Dma, Error, Fetch, Flow, PrepareChunk, Publish,
    ReadCopy, Retire, WriteCopy,
};

fn execute(device: &mut Dma<SharedMemory>) -> TestResult<Flow<Completion>> {
    let mut pipeline = Then::new(
        pdma_model_dma::CommandExecution::new(Then::new(
            PrepareChunk::default(),
            Then::new(ReadCopy::default(), WriteCopy::default()),
        )),
        CompletionInput,
    );
    let request = device.command_request(ChannelId::new(0))?;
    let StepOutcome::Complete(Ok(fetched)) = Operation::new(request).step(&mut Fetch, device)?
    else {
        return Err("fetch did not complete".into());
    };
    let expected_length = fetched
        .data()
        .ok_or("fetched command")?
        .command()
        .copy_length;
    let mut context = device.execution_context();
    let checked = super::support::finish(
        &mut CheckRanges::default(),
        &mut Operation::new(fetched),
        &mut context,
    )??;
    let input = context.install_progress(checked);
    let completion =
        super::support::finish(&mut pipeline, &mut Operation::new(input), &mut context)??;
    completion.status()?;
    assert_eq!(
        completion
            .data()
            .ok_or("command completion")?
            .response()
            .bytes_completed(),
        expected_length
    );
    Ok(completion)
}

fn setup() -> TestResult<Dma<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x2000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory, 2)?;
    for index in 0..2 {
        configure(&mut device, index)?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    Ok(device)
}

#[test]
fn reset_between_shared_execution_and_response_dispatch_rejects_stale_output() -> TestResult {
    for suspend_publication in [false, true] {
        // Given: shared execution has finished but its tagged response has not been dispatched.
        let mut device = setup()?;
        let response = execute(&mut device)?;
        assert_eq!(
            response.data().ok_or("response")?.channel(),
            ChannelId::new(0)
        );
        let mut publication = Some(response);
        let mut destinations = device.responses();
        if suspend_publication {
            device.write(
                ChannelId::new(0).register_offset(2).ok_or("offset")?
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::ENABLE.offset(),
                &0_u32.to_le_bytes(),
            )?;
            for _ in 0..16 {
                assert!(matches!(
                    destinations.step(&mut device.execution_context(), &mut publication,)?,
                    StepOutcome::InProgress
                ));
            }
        }
        // When: software requests reset while the old publication remains retained.
        device.write(
            ChannelId::new(0).admin_offset(2).ok_or("offset")?,
            &1_u32.to_le_bytes(),
        )?;
        let mut outcome = None;
        for _ in 0..256 {
            if let StepOutcome::Complete(result) =
                destinations.step(&mut device.execution_context(), &mut publication)?
            {
                outcome = Some(result);
                break;
            }
        }
        let outcome = outcome.ok_or("publication exceeded interaction bound")??;
        // Then: generation checking rejects the retained result without advancing either response ring.
        assert_eq!(outcome.status(), Err(Error::Reset));
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(0).admin_offset(2).ok_or("offset")?
            )?,
            1
        );
        assert_eq!(
            Operation::new(outcome).step(&mut Retire, &mut device)?,
            StepOutcome::Complete(Err(Error::Reset))
        );
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(0).admin_offset(2).ok_or("offset")?
            )?,
            0
        );
        for index in 0..2 {
            assert_eq!(
                word(
                    &mut device,
                    ChannelId::new(index).register_offset(2).ok_or("offset")?
                        + ChannelBlock::RESPONSE.offset()
                        + ResponseRegisters::PRODUCER.offset()
                )?,
                0
            );
        }
    }
    Ok(())
}

#[test]
fn a_response_port_rejects_work_owned_by_another_channel() -> TestResult {
    // Given: a completed response from channel zero and a port bound to channel one.
    let mut device = setup()?;
    let response = execute(&mut device)?;
    let mut port = Publish::new(ChannelId::new(1));
    // When: an incorrectly wired graph sends the response to the wrong port.
    let outcome = Operation::new(response).step(&mut port, &mut device.execution_context())?;
    // Then: routing misuse cannot publish into either channel's ring.
    let StepOutcome::Complete(Ok(outcome)) = outcome else {
        return Err("publication did not complete".into());
    };
    assert_eq!(outcome.status(), Err(Error::Routing));
    assert_eq!(
        Operation::new(outcome).step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Err(Error::Routing))
    );
    for index in 0..2 {
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(index).register_offset(2).ok_or("offset")?
                    + ChannelBlock::RESPONSE.offset()
                    + ResponseRegisters::PRODUCER.offset()
            )?,
            0
        );
    }
    Ok(())
}
