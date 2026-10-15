//! Typed payloads connect independently composable execution stages.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{
    HardwareDriver, Operation, RunError, Stage, StepOutcome, StepResult, Then,
};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{
    CheckRanges, Error, ExecutionContext, ExecutionError, Payload, PrepareChunk, Publish, ReadCopy,
    WriteCopy,
};
use support::{TestResult, command, setup, submit};

struct InvertPayload;

impl Stage<ExecutionContext<'_, SharedMemory>> for InvertPayload {
    type Input = Option<Payload>;
    type Output = Option<Payload>;
    type Error = ExecutionError;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, SharedMemory>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, ExecutionError> {
        let mut payload = state.take().ok_or(RunError::Exhausted)?;
        if let Some(payload) = &mut payload {
            match payload.bytes_mut() {
                Ok(bytes) => bytes.iter_mut().for_each(|byte| *byte = !*byte),
                Err(error) => return Ok(StepOutcome::Complete(Err(error.into()))),
            }
        }
        Ok(StepOutcome::Complete(Ok(payload)))
    }
}

#[test]
fn inserted_transform_changes_payload_without_new_executor_or_device_core() -> TestResult {
    // Given: a transform consumes the read stage's owned payload.
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[0, 1, 2, 255],
    )?;
    submit(&mut device, &memory, command())?;
    let read = Then::new(PrepareChunk, Then::new(ReadCopy, InvertPayload));
    let write = Then::new(WriteCopy, device.responses());
    let mut hardware = device.service_with(Then::new(read, write));
    let mut driver = HardwareDriver::new();
    // When: ordinary stages pass actual payload data through the inserted transform.
    for _ in 0..128 {
        assert_eq!(
            driver.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        );
        if device.irq() {
            break;
        }
    }
    // Then: the write uses transformed bytes and normal publication delivers its IRQ.
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [255, 254, 253, 0]);
    assert!(device.irq());
    Ok(())
}

#[test]
fn independently_composed_execution_obeys_device_fatal() -> TestResult {
    // Given: a freshly fetched command transferred out of the acceptance stage.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let fetched = support::fetch(&mut device, ChannelId::new(0))?;
    device.signal_fatal();
    // When: validation receives that command through a newly borrowed narrow context.
    let result = Operation::new(Some(fetched)).step(
        &mut CheckRanges,
        &mut device.execution_context(ChannelId::new(0))?,
    )?;
    // Then: global fatal still invalidates work without exposing the whole DMA.
    assert!(matches!(
        result,
        StepOutcome::Complete(Err(ExecutionError::Rejected(Error::Halted)))
    ));
    Ok(())
}

#[test]
fn rejected_publication_consumes_its_transient_stage_state() -> TestResult {
    // Given: a real completion sent to a publication port for the wrong channel.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let channel = ChannelId::new(0);
    let request = support::accept(&mut device, channel)?;
    let mut execution = Then::new(PrepareChunk, Then::new(ReadCopy, WriteCopy));
    let completion = {
        let mut context = device.execution_context(channel)?;
        let mut operation = Operation::new(Some(request));
        let mut completion = None;
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(&mut execution, &mut context)? {
                completion = result?;
                break;
            }
        }
        completion.ok_or("missing completion")?
    };
    let mut publish = Publish::new(ChannelId::new(1));
    let mut state = <Publish as Stage<ExecutionContext<'_, SharedMemory>>>::start(completion);
    let mut context = device.execution_context(channel)?;
    // When: publication rejects the actual completion's destination.
    assert_eq!(
        publish.step(&mut context, &mut state)?,
        StepOutcome::Complete(Err(ExecutionError::Rejected(Error::Routing)))
    );
    // Then: the terminal state remains exhausted even without Operation owning it.
    assert_eq!(
        publish.step(&mut context, &mut state),
        Err(RunError::Exhausted)
    );
    Ok(())
}
