//! Terminal command failures bypass successful chunk and payload dataflow.

use pdma_model_contract::dma::{
    ChannelBlock, Command, CopyCommandSourceValues, Event, EventCode, EventRaw, EventRegisters,
    LinearEndpoint, ScatterGatherEndpoint,
};

/// Shared hardware fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, Stage, StepOutcome, StepResult, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_dma::{
    Chunk, Completion, ExecutionContext, Payload, PrepareChunk, ReadCopy, WriteCopy,
};
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use support::{TestResult, command, read_word, setup, submit};

struct Observe<T>(Rc<Cell<(usize, usize)>>, PhantomData<T>);
impl<T> Stage<ExecutionContext<'_, SharedMemory>> for Observe<T> {
    type Input = pdma_model_dma::Flow<T>;
    type Output = pdma_model_dma::Flow<T>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, SharedMemory>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(value) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let (data, visits) = self.0.get();
        self.0
            .set((data + usize::from(value.data().is_some()), visits + 1));
        Ok(StepOutcome::Complete(Ok(value)))
    }
}

#[test]
fn failed_read_bypasses_payload_transform_and_reaches_terminal_tail() -> TestResult {
    check(
        {
            let original = command();
            let pdma_model_contract::dma::Command::Copy(copy) = original else {
                return Err("copy fixture required".into());
            };
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: original.id(),
                source: CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: 0x8000,
                    offset: 0,
                }),
                ..copy
            })
        },
        Some(EventCode::ReadError),
        (1, 0),
    )
}

#[test]
fn invalid_range_bypasses_normal_chunk_and_payload_outputs() -> TestResult {
    check(
        {
            let original = command();
            let pdma_model_contract::dma::Command::Copy(copy) = original else {
                return Err("copy fixture required".into());
            };
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: original.id(),
                source: CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: u64::MAX,
                    offset: 0,
                }),
                ..copy
            })
        },
        Some(EventCode::InvalidLength),
        (0, 0),
    )
}

#[test]
fn invalid_list_bypasses_normal_chunk_and_payload_outputs() -> TestResult {
    check(
        {
            let original = command();
            let pdma_model_contract::dma::Command::Copy(copy) = original else {
                return Err("copy fixture required".into());
            };
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: original.id(),
                source: CopyCommandSourceValues::ScatterGather(ScatterGatherEndpoint {
                    list_address: 0x8000,
                    count: 1,
                    index: 0,
                    offset: 0,
                }),
                ..copy
            })
        },
        Some(EventCode::ListReadError),
        (0, 0),
    )
}

#[test]
fn empty_success_is_normal_dataflow_without_payload_access() -> TestResult {
    check(
        {
            let original = command();
            let pdma_model_contract::dma::Command::Copy(copy) = original else {
                return Err("copy fixture required".into());
            };
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: original.id(),
                source: CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: 0x8000,
                    offset: 0,
                }),
                copy_length: 0,
                ..copy
            })
        },
        None,
        (1, 1),
    )
}

#[test]
fn standalone_chunk_graph_publishes_the_actual_read_failure() -> TestResult {
    // Given: accepted traversal whose payload source is inaccessible.
    let (mut dma, memory) = setup(0x1100, 2)?;
    let descriptor = {
        let original = command();
        let pdma_model_contract::dma::Command::Copy(copy) = original else {
            return Err("copy fixture required".into());
        };
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: original.id(),
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x8000,
                offset: 0,
            }),
            ..copy
        })
    };
    submit(&mut dma, &memory, descriptor)?;
    let id = pdma_model_contract::dma::ChannelId::new(0);
    let input = support::accept(&mut dma, id)?;
    let mut graph = support::chunk_pipeline();
    // When: the standalone graph runs without a Hardware controller.
    let retired = {
        let mut context = dma.execution_context();
        let mut operation = hardware_interface_pipeline::Operation::new(input);
        let mut output = None;
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(&mut graph, &mut context)? {
                output = Some(result?);
                break;
            }
        }
        assert!(operation.is_complete());
        output.ok_or("standalone graph watchdog")?
    };
    assert_eq!(
        hardware_interface_pipeline::Operation::new(vec![(0, retired)])
            .step(&mut pdma_model_dma::Retire, &mut dma)?,
        StepOutcome::Complete(Err(pdma_model_dma::Error::Halted))
    );
    // Then: one actual failure event publishes with no fabricated success bytes.
    assert_eq!(
        read_word(
            &mut dma,
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

fn check(
    descriptor: Command,
    expected_status: Option<EventCode>,
    expected_counts: (usize, usize),
) -> TestResult {
    // Given: real descriptor memory and observable successful-data/completion boundaries.
    let (mut dma, memory) = setup(0x1100, 2)?;
    memory.write(0x1300.into(), MemoryAlignment::ONE, &[0xaa; 4])?;
    submit(&mut dma, &memory, descriptor)?;
    let chunks = Rc::new(Cell::new((0, 0)));
    let payloads = Rc::new(Cell::new((0, 0)));
    let completions = Rc::new(Cell::new((0, 0)));
    let read = Then::new(
        PrepareChunk::default(),
        Then::new(
            Observe::<Chunk>(Rc::clone(&chunks), PhantomData),
            Then::new(
                ReadCopy::default(),
                Observe::<Payload>(Rc::clone(&payloads), PhantomData),
            ),
        ),
    );
    let tail = Observe::<Completion>(Rc::clone(&completions), PhantomData);
    let mut hardware = dma.hardware_with(Then::new(
        pdma_model_dma::CommandExecution::new(Then::new(read, WriteCopy::default())),
        tail,
    ));
    let mut driver = Operation::new(());
    let producer =
        ChannelBlock::ALIGNMENT + ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset();
    // When: the real pipeline reaches its event publication boundary.
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = driver.step(&mut hardware, &mut dma)? {
            result?;
            break;
        }
    }
    // Then: failures expose no fabricated successful data or completion receipt.
    assert!(driver.is_complete());
    assert_eq!(
        read_word(&mut dma, producer)?,
        u32::from(expected_status.is_some())
    );
    assert_eq!(
        completions.get(),
        (usize::from(expected_status.is_none()), 1)
    );
    assert_eq!((chunks.get().0, payloads.get().0), expected_counts);
    if let Some(code) = expected_status {
        let raw: EventRaw = memory.read_typed(0x1100.into(), MemoryAlignment::ONE)?;
        let event = Event::from_bytes(raw.as_ref())?;
        assert_eq!(
            (event.id, event.code, event.bytes_completed()),
            (descriptor.id(), code, 0)
        );
    }
    assert_eq!(
        memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?,
        [0xaa; 4]
    );
    assert_eq!(dma.irq(), expected_status.is_some());
    Ok(())
}
