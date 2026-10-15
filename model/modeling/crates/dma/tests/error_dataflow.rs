//! Terminal command failures bypass successful chunk and payload dataflow.
/// Shared hardware fixtures.
pub mod support;

use hardware_interface_pipeline::{HardwareDriver, RunError, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, Command, Endpoint, Response, ResponseRaw, ResponseRegisters, ResponseStatus,
};
use pdma_model_dma::{
    Chunk, Completion, ExecutionContext, ExecutionError, Payload, PrepareChunk, ReadCopy, WriteCopy,
};
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use support::{TestResult, command, read_word, setup, submit};

struct Observe<T>(Rc<Cell<usize>>, PhantomData<T>);
impl<T> Stage<ExecutionContext<'_, SharedMemory>> for Observe<T> {
    type Input = Option<T>;
    type Output = Option<T>;
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
        let value = state.take().ok_or(RunError::Exhausted)?;
        if value.is_some() {
            self.0.set(self.0.get() + 1);
        }
        Ok(StepOutcome::Complete(Ok(value)))
    }
}

#[test]
fn failed_read_bypasses_payload_transform_but_keeps_completion_tail() -> TestResult {
    check(
        Command {
            source: Endpoint::linear(0x8000, 0),
            ..command()
        },
        ResponseStatus::ReadError,
        (1, 0),
    )
}

#[test]
fn invalid_range_bypasses_normal_chunk_and_payload_outputs() -> TestResult {
    check(
        Command {
            source: Endpoint::linear(u64::MAX, 0),
            ..command()
        },
        ResponseStatus::InvalidLength,
        (0, 0),
    )
}

#[test]
fn invalid_list_bypasses_normal_chunk_and_payload_outputs() -> TestResult {
    check(
        Command {
            source: Endpoint::scatter_gather(0x8000, 1),
            ..command()
        },
        ResponseStatus::ListReadError,
        (0, 0),
    )
}

#[test]
fn empty_success_is_normal_dataflow_without_payload_access() -> TestResult {
    check(
        Command {
            source: Endpoint::linear(0x8000, 0),
            copy_length: 0,
            ..command()
        },
        ResponseStatus::Success,
        (1, 1),
    )
}

fn check(
    descriptor: Command,
    expected_status: ResponseStatus,
    expected_counts: (usize, usize),
) -> TestResult {
    // Given: real descriptor memory and observable successful-data/completion boundaries.
    let (mut dma, memory) = setup(0x1100, 2)?;
    memory.write(0x1300.into(), MemoryAlignment::ONE, &[0xaa; 4])?;
    submit(&mut dma, &memory, descriptor)?;
    let chunks = Rc::new(Cell::new(0));
    let payloads = Rc::new(Cell::new(0));
    let completions = Rc::new(Cell::new(0));
    let read = Then::new(
        PrepareChunk,
        Then::new(
            Observe::<Chunk>(Rc::clone(&chunks), PhantomData),
            Then::new(
                ReadCopy,
                Observe::<Payload>(Rc::clone(&payloads), PhantomData),
            ),
        ),
    );
    let tail = Then::new(
        Observe::<Completion>(Rc::clone(&completions), PhantomData),
        dma.responses(),
    );
    let mut service = dma.service_with(Then::new(read, Then::new(WriteCopy, tail)));
    let mut driver = HardwareDriver::new();
    let producer = ChannelBlock::ALIGNMENT
        + ChannelBlock::RESPONSE.offset()
        + ResponseRegisters::PRODUCER.offset();
    // When: the real pipeline reaches its response publication boundary.
    for _ in 0..256 {
        assert_eq!(
            driver.step(&mut service, &mut dma)?,
            StepOutcome::InProgress
        );
        if read_word(&mut dma, producer)? == 1 {
            break;
        }
    }
    // Then: failures expose no fabricated successful data, but all outcomes reach the same tail.
    assert_eq!(read_word(&mut dma, producer)?, 1);
    assert_eq!(completions.get(), 1);
    assert_eq!((chunks.get(), payloads.get()), expected_counts);
    let raw: ResponseRaw = memory.read_typed(0x1100.into(), MemoryAlignment::ONE)?;
    let response = Response::from_bytes(raw.as_ref())?;
    assert_eq!(
        (response.id, response.status, response.bytes_completed()),
        (descriptor.id, expected_status, 0)
    );
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [0xaa; 4]
    );
    assert!(dma.irq());
    Ok(())
}
