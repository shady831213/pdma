use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome, Then};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, EventRegisters};
use pdma_model_dma::{
    CheckRanges, Completion, Dma, Error, Fetch, Flow, PrepareChunk, ReadCopy, Retire, WriteCopy,
};

fn execute(device: &mut Dma<SharedMemory>, index: usize) -> TestResult<Flow<Completion>> {
    let mut pipeline = pdma_model_dma::CommandExecution::new(Then::new(
        PrepareChunk::default(),
        Then::new(ReadCopy::default(), WriteCopy::default()),
    ));
    let request = device.command_request(ChannelId::new(index))?;
    let StepOutcome::Complete(Ok(fetched)) = Operation::new(request).step(&mut Fetch, device)?
    else {
        return Err("fetch did not complete".into());
    };
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
        completion.data().ok_or("command completion")?.channel(),
        ChannelId::new(index)
    );
    let mut bytes = [0; 512];
    device.memory.read(
        MemoryAddress::new(0x1400 + u64::try_from(index)? * 0x1000),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    assert_eq!(bytes, [7; 512]);
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
        device.memory.write(
            MemoryAddress::new(0x1200 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &[7; 512],
        )?;
        configure(&mut device, index)?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    Ok(device)
}

#[test]
fn reset_between_shared_execution_and_retirement_rejects_stale_output() -> TestResult {
    for disable_owner in [false, true] {
        // Given: execution has finished but its owner remains resident until retirement.
        let mut device = setup()?;
        let completion = execute(&mut device, 0)?;
        if disable_owner {
            device.write(
                ChannelId::new(0).register_offset(2).ok_or("offset")?
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::ENABLE.offset(),
                &0_u32.to_le_bytes(),
            )?;
        }
        // When: software requests reset while the old completion remains retained.
        device.write(
            ChannelId::new(0).admin_offset(2).ok_or("offset")?,
            &1_u32.to_le_bytes(),
        )?;
        // Then: generation checking rejects the retained result without advancing either event ring.
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(0).admin_offset(2).ok_or("offset")?
            )?,
            1
        );
        assert_eq!(
            Operation::new(vec![(0, completion)]).step(&mut Retire, &mut device)?,
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
                        + ChannelBlock::EVENT.offset()
                        + EventRegisters::PRODUCER.offset()
                )?,
                0
            );
        }
    }
    Ok(())
}

#[test]
fn retirement_rejects_a_completion_owned_by_another_channel() -> TestResult {
    // Given: a completion value from channel zero retained after its flow retires.
    let mut device = setup()?;
    let mut foreign = None;
    let original = execute(&mut device, 0)?
        .map(|completion| {
            foreign = Some(completion);
        })
        .finish();
    assert_eq!(
        Operation::new(vec![(0, original)]).step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Ok(()))
    );
    let current = execute(&mut device, 1)?;
    let foreign = foreign.ok_or("foreign completion")?;
    // When: an incorrectly wired graph substitutes that value into channel one's flow.
    let outcome = current.map(|_| foreign);
    // Then: retirement rejects the mismatch without publishing into either event ring.
    assert_eq!(
        Operation::new(vec![(0, outcome)]).step(&mut Retire, &mut device)?,
        StepOutcome::Complete(Err(Error::Routing))
    );
    assert!(device.command_request(ChannelId::new(1))?.is_some());
    for index in 0..2 {
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(index).register_offset(2).ok_or("offset")?
                    + ChannelBlock::EVENT.offset()
                    + EventRegisters::PRODUCER.offset()
            )?,
            0
        );
    }
    Ok(())
}
