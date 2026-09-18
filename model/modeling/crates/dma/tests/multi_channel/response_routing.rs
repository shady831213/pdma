use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::{ChannelBlock, ChannelId, ChannelRegisters, ResponseRegisters};
use pdma_model_dma::{Dma, Error, Publish, Resettable};

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
        let mut execution = super::support::single_command(&device).upstream;
        let mut operation = Operation::new(());
        let response = (0..128)
            .find_map(|_| match operation.step(&mut execution, &mut device) {
                StepOutcome::Complete(result) => Some(result),
                _ => None,
            })
            .ok_or("execution did not finish")??;
        assert_eq!(response.channel(), ChannelId::new(0));
        let mut publication = Operation::new(response);
        let mut destinations = device.responses();
        if suspend_publication {
            device.write(
                ChannelId::new(0).register_offset(2).ok_or("offset")?
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::ENABLE.offset(),
                &0_u32.to_le_bytes(),
            )?;
            let suspended = (0..8)
                .map(|_| publication.step(&mut destinations, &mut device))
                .find(|outcome| !matches!(outcome, StepOutcome::Progressed))
                .ok_or("publication never suspended")?;
            assert!(matches!(suspended, StepOutcome::Pending(())));
        }
        // When: software requests reset while the old publication remains retained.
        device.write(
            ChannelId::new(0).admin_offset(2).ok_or("offset")?,
            &1_u32.to_le_bytes(),
        )?;
        let outcome = (0..8)
            .map(|_| publication.step(&mut destinations, &mut device))
            .find(|outcome| !matches!(outcome, StepOutcome::Progressed))
            .ok_or("publication never terminated")?;
        // Then: generation checking rejects the retained result without advancing either response ring.
        assert!(matches!(outcome, StepOutcome::Complete(Err(Error::Reset))));
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
    let mut execution = super::support::single_command(&device).upstream;
    let mut operation = Operation::new(());
    let response = (0..128)
        .find_map(|_| match operation.step(&mut execution, &mut device) {
            StepOutcome::Complete(result) => Some(result),
            _ => None,
        })
        .ok_or("execution did not finish")??;
    let mut port = Resettable::for_channel(ChannelId::new(1), Publish);
    // When: an incorrectly wired graph sends the response to the wrong port.
    let outcome = Operation::new(response).step(&mut port, &mut device);
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
