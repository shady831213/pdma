use super::*;
use hardware_interface_pipeline::RunError;
use pdma_model_contract::dma::{ChannelBlock, Endpoint, EventRegisters};
use pdma_model_dma::{CommandExecution, Completion, Flow};
use std::{cell::Cell, rc::Rc};

struct InterruptedChunk {
    pending: Option<Flow<Completion>>,
    interrupted: bool,
    calls: Rc<Cell<usize>>,
}

impl Stage<ExecutionContext<'_, SharedMemory>> for InterruptedChunk {
    type Input = Flow<Completion>;
    type Output = Flow<Completion>;
    type Error = std::convert::Infallible;

    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, SharedMemory>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.pending.is_none() {
            self.pending = input.take();
            if self.pending.is_some() {
                self.calls.set(self.calls.get() + 1);
            }
        }
        let Some(pending) = self.pending.as_ref() else {
            return Ok(StepOutcome::InProgress);
        };
        if !self.interrupted {
            self.interrupted = true;
            return Err(RunError::Exhausted);
        }
        let lifecycle = pending.lifecycle(context);
        let completion = self.pending.take().ok_or(RunError::Exhausted)?;
        Ok(StepOutcome::Complete(Ok(match lifecycle {
            Ok(()) => completion,
            Err(error) => completion.stop(error),
        })))
    }
}

struct CommandTail {
    calls: Rc<Cell<usize>>,
    memory: SharedMemory,
    final_address: hardware_interface_runtime::MemoryAddress,
}

impl Stage<ExecutionContext<'_, SharedMemory>> for CommandTail {
    type Input = Flow<Completion>;
    type Output = Flow<Completion>;
    type Error = std::convert::Infallible;

    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, SharedMemory>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(completion) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        self.calls.set(self.calls.get() + 1);
        assert_eq!(
            completion.data().map(Completion::channel),
            Some(ChannelId::new(0))
        );
        let byte = self
            .memory
            .read_typed::<[u8; 1]>(self.final_address, MemoryAlignment::ONE)
            .map_err(|_| RunError::Exhausted)?;
        assert_eq!(byte, [0xa5]);
        Ok(StepOutcome::Complete(Ok(completion)))
    }
}

#[test]
fn custom_command_tail_receives_only_final_completion_for_multiple_chunks() -> TestResult {
    // Given: a transformed multi-chunk command followed by a command-level observer.
    let (mut device, memory) = setup(0x1100, 2)?;
    let source = [0x5a; 600];
    let descriptor = {
        let original = command();
        let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) = original.payload else {
            return Err("copy fixture required".into());
        };
        pdma_model_contract::dma::Command::copy(
            original.id,
            pdma_model_contract::dma::CopyCommand {
                copy_length: u64::try_from(source.len())?,
                source: Endpoint::linear(0x1400, 0),
                destination: Endpoint::linear(0x1800, 0),
                ..copy
            },
        )
    };
    memory.write(0x1400.into(), MemoryAlignment::ONE, &source)?;
    submit(&mut device, &memory, descriptor)?;
    let calls = Rc::new(Cell::new(0));
    let chunks = Then::new(
        PrepareChunk::default(),
        Then::new(
            ReadCopy::default(),
            Then::new(InvertPayload, WriteCopy::default()),
        ),
    );
    let pipeline = Then::new(
        CommandExecution::new(chunks),
        CommandTail {
            calls: Rc::clone(&calls),
            memory: memory.clone(),
            final_address: (0x1800 + u64::try_from(source.len() - 1)?).into(),
        },
    );
    let mut hardware = device.hardware_with(pipeline);
    let mut operation = Operation::new(());
    // When: one finite hardware invocation runs to its terminal result.
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
            break;
        }
    }
    // Then: the tail sees one final completion and all transformed bytes are committed.
    assert!(operation.is_complete());
    assert_eq!(calls.get(), 1);
    let mut destination = [0; 600];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut destination)?;
    assert_eq!(destination, source.map(|byte| !byte));
    assert_eq!(
        support::read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        0
    );

    Ok(())
}

#[test]
fn raw_child_error_retains_command_and_resumes_without_replaying_committed_chunk() -> TestResult {
    // Given: a chunk-local stage consumes its first result before returning a raw error.
    let (mut device, memory) = setup(0x1100, 2)?;
    let descriptor = {
        let original = command();
        let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) = original.payload else {
            return Err("copy fixture required".into());
        };
        pdma_model_contract::dma::Command::copy(
            original.id,
            pdma_model_contract::dma::CopyCommand {
                copy_length: 600,
                source: Endpoint::linear(0x1400, 0),
                destination: Endpoint::linear(0x1800, 0),
                ..copy
            },
        )
    };
    memory.write(0x1400.into(), MemoryAlignment::ONE, &[0x5a; 600])?;
    submit(&mut device, &memory, descriptor)?;
    let calls = Rc::new(Cell::new(0));
    let chunks = Then::new(
        PrepareChunk::default(),
        Then::new(
            ReadCopy::default(),
            Then::new(
                WriteCopy::default(),
                InterruptedChunk {
                    pending: None,
                    interrupted: false,
                    calls: Rc::clone(&calls),
                },
            ),
        ),
    );
    let pipeline = CommandExecution::new(chunks);
    let mut hardware = device.hardware_with(pipeline);
    let mut operation = Operation::new(());
    let mut interrupted = false;
    for _ in 0..1024 {
        match operation.step(&mut hardware, &mut device) {
            Err(RunError::Exhausted) => {
                interrupted = true;
                break;
            }
            outcome => {
                let _ = outcome?;
            }
        }
    }
    assert!(interrupted);
    assert!(!operation.is_complete());
    assert!(device.command_request(ChannelId::new(0))?.is_none());
    assert_eq!(calls.get(), 1);
    let mut prefix = [0; pdma_model_dma::COPY_CHUNK_BYTES];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut prefix)?;
    assert_eq!(prefix, [0x5a; pdma_model_dma::COPY_CHUNK_BYTES]);
    // When: source bytes change and the same retained operation resumes.
    memory.write(
        0x1400.into(),
        MemoryAlignment::ONE,
        &[0xaa; pdma_model_dma::COPY_CHUNK_BYTES],
    )?;
    for _ in 0..1024 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
            break;
        }
    }
    // Then: all chunks complete once and the committed prefix is never reread or rewritten.
    assert!(operation.is_complete());
    assert_eq!(
        calls.get(),
        usize::try_from(crate::support::copy(&descriptor)?.copy_length)?
            .div_ceil(pdma_model_dma::COPY_CHUNK_BYTES)
    );
    let mut destination = [0; 600];
    memory.read(0x1800.into(), MemoryAlignment::ONE, &mut destination)?;
    assert_eq!(destination, [0x5a; 600]);
    assert_eq!(
        support::read_word(
            &mut device,
            ChannelBlock::ALIGNMENT
                + ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        0
    );
    Ok(())
}
