use super::support::{TestResult, configure, finish, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, ResponseRegisters};
use pdma_model_dma::{COPY_CHUNK_BYTES, Candidate, Dma, Error, Publish, Work};

fn execute(device: &mut Dma<SharedMemory>) -> TestResult<Work> {
    let pipeline = super::support::single_chunk(device);
    let mut execution = Then::new(
        Candidate::new(ChannelId::new(0)),
        Then::new(pipeline.upstream, pipeline.downstream.upstream),
    );
    let mut work = None;
    for _ in 0..512_usize.div_ceil(COPY_CHUNK_BYTES) {
        let mut operation = Operation::new(());
        work = finish(&mut execution, &mut operation, device)??;
    }
    work.ok_or_else(|| "command did not produce response work".into())
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
        assert_eq!(response.channel(), ChannelId::new(0));
        let mut publication = Operation::new(Some(response));
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
                    publication.step(&mut destinations, &mut device)?,
                    StepOutcome::InProgress
                ));
            }
        }
        // When: software requests reset while the old publication remains retained.
        device.write(
            ChannelId::new(0).admin_offset(2).ok_or("offset")?,
            &1_u32.to_le_bytes(),
        )?;
        let outcome = finish(&mut destinations, &mut publication, &mut device)?;
        // Then: generation checking rejects the retained result without advancing either response ring.
        assert!(matches!(outcome, Err(Error::Reset)));
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(0).admin_offset(2).ok_or("offset")?
            )?,
            1
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
    let outcome = Operation::new(response).step(&mut port, &mut device)?;
    // Then: routing misuse cannot publish into either channel's ring.
    assert!(matches!(
        outcome,
        StepOutcome::Complete(Err(Error::Routing))
    ));
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
