//! Typed payloads connect independently composable execution stages.

use pdma_model_contract::dma::{CopyCommandSourceValues, LinearEndpoint};

mod command_tail;
/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_memory::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use hardware_interface_pipeline::{
    HardwareDriver, Operation, Stage, StepOutcome, StepResult, Then,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Event, EventCode, EventRaw, EventRegisters,
};
use pdma_model_dma::{
    CheckRanges, Error, ExecutionContext, Payload, PrepareChunk, ReadCopy, WriteCopy,
};
use support::{TestResult, command, setup, submit};

struct InvertPayload;

impl Stage<ExecutionContext<'_, SharedMemory>> for InvertPayload {
    type Input = pdma_model_dma::Flow<Payload>;
    type Output = pdma_model_dma::Flow<Payload>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, SharedMemory>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(mut payload) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        if let Some(data) = payload.data_mut() {
            match data.bytes_mut() {
                Ok(bytes) => bytes.iter_mut().for_each(|byte| *byte = !*byte),
                Err(error) => return Ok(StepOutcome::Complete(Ok(payload.stop(error)))),
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
    let read = Then::new(
        PrepareChunk::default(),
        Then::new(ReadCopy::default(), InvertPayload),
    );
    let pipeline = pdma_model_dma::CommandExecution::new(Then::new(read, WriteCopy::default()));
    let mut hardware = device.hardware_with(pipeline);
    let mut driver = HardwareDriver::new();
    // When: ordinary stages pass actual payload data through the inserted transform.
    for _ in 0..128 {
        assert_eq!(
            driver.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        );
        if memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)? == [255, 254, 253, 0]
        {
            break;
        }
    }
    // Then: the write uses transformed bytes without producing an error event or IRQ.
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [255, 254, 253, 0]);
    assert!(!device.irq());
    support::assert_no_event(&mut device)?;
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
    let result = Operation::new(fetched)
        .step(&mut CheckRanges::default(), &mut device.execution_context())?;
    // Then: global fatal still invalidates work without exposing the whole DMA.
    assert!(
        matches!(result, StepOutcome::Complete(Ok(flow)) if flow.status() == Err(Error::Halted))
    );
    Ok(())
}

#[test]
fn failed_read_consumes_its_transient_state_without_repeating_the_event() -> TestResult {
    // Given: a real chunk whose source memory is inaccessible.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut descriptor = command();
    support::copy_mut(&mut descriptor)?.source = CopyCommandSourceValues::Linear(LinearEndpoint {
        address: 0x9000,
        offset: 0,
    });
    submit(&mut device, &memory, descriptor)?;
    let channel = ChannelId::new(0);
    let request = support::accept(&mut device, channel)?;
    let chunk = {
        let mut context = device.execution_context();
        let mut operation = Operation::new(request);
        let mut prepare = PrepareChunk::default();
        let mut chunk = None;
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(&mut prepare, &mut context)? {
                chunk = Some(result?);
                break;
            }
        }
        chunk.ok_or("missing chunk")?
    };
    let mut read = ReadCopy::default();
    let mut state = Some(chunk);
    // When: the owner reports the read failure at its originating stage.
    assert!(
        matches!(read.step(&mut device.execution_context(), &mut state)?, StepOutcome::Complete(Ok(flow)) if flow.status() == Err(Error::Halted))
    );
    // Then: consumed input cannot replay the memory access or report a second event.
    assert!(state.is_none());
    assert!(matches!(
        read.step(&mut device.execution_context(), &mut state),
        Ok(StepOutcome::InProgress)
    ));
    assert_eq!(
        support::read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        1
    );
    let raw: EventRaw = memory.read_typed(0x1100.into(), MemoryAlignment::ONE)?;
    let event = Event::from_bytes(raw.as_ref())?;
    assert_eq!(
        (event.id, event.code, event.bytes_completed()),
        (descriptor.id(), EventCode::ReadError, 0)
    );
    Ok(())
}
