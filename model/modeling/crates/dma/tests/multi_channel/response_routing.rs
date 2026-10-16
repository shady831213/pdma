use super::support::{TestResult, configure, submit, word};
use hardware_interface_pipeline::{
    HardwareDriver, Operation, RunError, Stage, StepOutcome, StepResult, Then,
};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{ChannelBlock, ChannelId, ChannelRegisters, ResponseRegisters};
use pdma_model_dma::{
    Completion, Dma, Error, ExecutionContext, PrepareChunk, Publish, ReadCopy, Responses, WriteCopy,
};
use std::{cell::RefCell, rc::Rc};

struct CaptureCompletion(Rc<RefCell<Option<Completion>>>);

impl Stage<ExecutionContext<'_, SharedMemory>> for CaptureCompletion {
    type Input = Option<Completion>;
    type Output = ();
    type Error = Error;
    type State = Option<Self::Input>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, SharedMemory>,
        state: &mut Self::State,
    ) -> StepResult<(), Error> {
        *self.0.borrow_mut() = state.take().ok_or(RunError::Exhausted)?;
        Ok(StepOutcome::Complete(Ok(())))
    }
}

fn execute(device: &mut Dma<SharedMemory>) -> TestResult<Completion> {
    let captured = Rc::new(RefCell::new(None));
    let read = Then::new(PrepareChunk, ReadCopy);
    let write = Then::new(
        WriteCopy,
        Then::new(
            pdma_model_dma::CompletionInput,
            CaptureCompletion(Rc::clone(&captured)),
        ),
    );
    let mut hardware = device.hardware_with(Then::new(read, write));
    let mut driver = HardwareDriver::new();
    for _ in 0..256 {
        assert_eq!(driver.step(&mut hardware, device)?, StepOutcome::InProgress);
        if let Some(completion) = captured.borrow_mut().take() {
            assert_eq!(completion.response().bytes_completed(), 512);
            return Ok(completion);
        }
    }
    Err("command did not produce response work".into())
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
        let mut publication =
            <Responses as Stage<ExecutionContext<'_, SharedMemory>>>::start(Some(response));
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
                    destinations.step(
                        &mut device.execution_context(ChannelId::new(0))?,
                        &mut publication,
                    )?,
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
            if let StepOutcome::Complete(result) = destinations.step(
                &mut device.execution_context(ChannelId::new(0))?,
                &mut publication,
            )? {
                outcome = Some(result);
                break;
            }
        }
        let outcome = outcome.ok_or("publication exceeded interaction bound")?;
        // Then: generation checking rejects the retained result without advancing either response ring.
        assert!(matches!(outcome, Err(Error::Reset)));
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
    let outcome = Operation::new(response)
        .step(&mut port, &mut device.execution_context(ChannelId::new(1))?)?;
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
