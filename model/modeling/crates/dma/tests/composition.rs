//! A downstream stage can transform the copy payload without replacing execution.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, RunError, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_dma::{CheckRanges, Dma, Error, Fetch, NextChunk, ReadCopy, Work, WriteCopy};
use support::{TestResult, command, setup, submit};

struct InvertPayload;

impl Stage<Dma<SharedMemory>> for InvertPayload {
    type Input = Option<Work>;
    type Output = Option<Work>;
    type Error = Error;
    type State = Option<Option<Work>>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        device: &mut Dma<SharedMemory>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(work) = state.take() else {
            return Err(RunError::Exhausted);
        };
        if let Some(work) = &work {
            match device.work_scratch_mut(work) {
                Ok(Some(bytes)) => bytes.iter_mut().for_each(|byte| *byte = !*byte),
                Ok(None) => {}
                Err(error) => return Ok(StepOutcome::Complete(Err(error))),
            }
        }
        Ok(StepOutcome::Complete(Ok(work)))
    }
}

#[test]
fn inserted_transform_changes_payload_without_new_executor_or_device_core() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[0, 1, 2, 255],
    )?;
    submit(&mut device, &memory, command())?;
    let chunk = Then::new(
        Then::new(Then::new(NextChunk, ReadCopy), InvertPayload),
        WriteCopy,
    );
    let execution = Then::new(CheckRanges, chunk);
    let mut hardware = Then::new(Fetch, Then::new(execution, device.responses()));
    let mut operation =
        Operation::new(device.command_request(pdma_model_contract::dma::ChannelId::new(0))?);
    let mut complete = false;
    // When
    for _ in 0..32 {
        match operation
            .step(&mut hardware, &mut device)
            .map_err(|e| format!("{e:?}"))?
        {
            StepOutcome::InProgress => {}
            StepOutcome::Complete(result) => {
                result?;
                complete = true;
                break;
            }
        }
    }
    // Then
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert!(complete);
    assert_eq!(bytes, [255, 254, 253, 0]);
    assert!(device.irq());
    Ok(())
}

#[test]
fn independently_composed_execution_obeys_device_fatal() -> TestResult {
    // Given: an execution stage started with a valid command.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let request = device.command_request(pdma_model_contract::dma::ChannelId::new(0))?;
    let StepOutcome::Complete(Ok(work)) = Operation::new(request).step(&mut Fetch, &mut device)?
    else {
        return Err("command not accepted".into());
    };
    let mut execution = CheckRanges;
    let mut operation = Operation::new(work);
    device.signal_fatal();
    // When: the stage is driven without the default outer pipeline.
    let result = operation.step(&mut execution, &mut device)?;
    // Then: device fatal blocks even independently composed hardware.
    assert!(matches!(result, StepOutcome::Complete(Err(Error::Halted))));
    Ok(())
}

#[test]
fn rejected_publication_consumes_its_transient_stage_state() -> TestResult {
    // Given: an accepted descriptor whose hardware action has not reached response publication.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let channel = pdma_model_contract::dma::ChannelId::new(0);
    let request = device.command_request(channel)?;
    let StepOutcome::Complete(Ok(Some(work))) =
        Operation::new(request).step(&mut Fetch, &mut device)?
    else {
        return Err("command not accepted".into());
    };
    let mut publish = pdma_model_dma::Publish::new(channel);
    let mut state = <pdma_model_dma::Publish as Stage<Dma<SharedMemory>>>::start(work);
    // When: the standalone publication leaf rejects the premature work.
    assert_eq!(
        publish.step(&mut device, &mut state)?,
        StepOutcome::Complete(Err(Error::Routing))
    );
    // Then: terminal rejection consumes transient state even without an Operation wrapper.
    assert_eq!(
        publish.step(&mut device, &mut state),
        Err(RunError::Exhausted)
    );
    Ok(())
}
