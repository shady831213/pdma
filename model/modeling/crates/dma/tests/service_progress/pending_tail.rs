use crate::{
    PRODUCER,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{Stage, StepOutcome, Then};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelRegisters, CommandRegisters, Endpoint, ResponseRegisters,
};
use pdma_model_dma::{COPY_CHUNK_BYTES, PrepareChunk, ReadCopy, WriteCopy};
use std::{cell::Cell, rc::Rc};

struct Tail {
    entered: Rc<Cell<bool>>,
    release: Rc<Cell<bool>>,
    completed: Rc<Cell<usize>>,
    dropped: Rc<Cell<bool>>,
}

struct TailState {
    completion: Option<pdma_model_dma::Completion>,
    dropped: Option<Rc<Cell<bool>>>,
}

impl Drop for TailState {
    fn drop(&mut self) {
        if let Some(dropped) = &self.dropped {
            dropped.set(true);
        }
    }
}

impl<M> Stage<pdma_model_dma::ExecutionContext<'_, M>> for Tail {
    type Input = Option<pdma_model_dma::Completion>;
    type Output = Option<pdma_model_dma::Completion>;
    type Error = pdma_model_dma::Error;
    type State = TailState;

    fn start(input: Self::Input) -> Self::State {
        TailState {
            completion: input,
            dropped: None,
        }
    }

    fn step(
        &mut self,
        _: &mut pdma_model_dma::ExecutionContext<'_, M>,
        state: &mut Self::State,
    ) -> hardware_interface_pipeline::StepResult<Self::Output, Self::Error> {
        self.entered.set(true);
        state.dropped = Some(Rc::clone(&self.dropped));
        Ok(if self.release.get() {
            self.completed.set(self.completed.get() + 1);
            StepOutcome::Complete(Ok(state.completion.take()))
        } else {
            StepOutcome::InProgress
        })
    }
}

#[test]
fn reset_after_committed_chunk_retires_hardware_while_custom_tail_is_pending() -> TestResult {
    // Given: a non-final chunk published progress while a custom tail remains pending.
    let (mut device, memory) = setup(0x1100, 2)?;
    let descriptor = pdma_model_contract::dma::Command {
        copy_length: u64::try_from(COPY_CHUNK_BYTES * 2)?,
        source: Endpoint::linear(0x1400, 0),
        destination: Endpoint::linear(0x1800, 0),
        ..command()
    };
    submit(&mut device, &memory, descriptor)?;
    let entered = Rc::new(Cell::new(false));
    let release = Rc::new(Cell::new(false));
    let completed = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));
    let sink = Then::new(
        WriteCopy,
        Tail {
            entered: Rc::clone(&entered),
            release: Rc::clone(&release),
            completed: Rc::clone(&completed),
            dropped: Rc::clone(&dropped),
        },
    );
    let pipeline = Then::new(
        Then::new(PrepareChunk, ReadCopy),
        Then::new(
            sink.upstream,
            Then::new(
                pdma_model_dma::CompletionInput,
                Then::new(sink.downstream, device.responses()),
            ),
        ),
    );
    let mut hardware = device.hardware_with(pipeline);
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
        }
        if entered.get() {
            break;
        }
    }
    assert!(entered.get());
    // When: reset arrives after the write but before the tail's terminal step.
    let reset = pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CHANNELS_OFFSET;
    device.write(reset, &1_u32.to_le_bytes())?;
    assert_eq!(read_word(&mut device, reset)?, 1);
    assert!(!dropped.get());
    for _ in 0..12 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
        }
        if read_word(&mut device, reset)? == 0 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert!(
        dropped.get(),
        "reset acknowledged before resident tail was dropped"
    );
    assert!(!release.get());
    release.set(true);
    for _ in 0..128 {
        assert_eq!(
            operation.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        );
    }
    // Then: generic tail completion cannot publish or resurrect the retired command.
    assert_eq!(completed.get(), 0);
    assert!(!operation.is_complete());
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert_eq!(read_word(&mut device, PRODUCER)?, 0);
    reconfigure(&mut device)?;
    submit(&mut device, &memory, command())?;
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut device)? {
            result?;
        }
        if read_word(&mut device, PRODUCER)? == 1 {
            break;
        }
    }
    assert_eq!(read_word(&mut device, PRODUCER)?, 1);
    assert_eq!(completed.get(), 1);
    Ok(())
}

fn reconfigure<M>(device: &mut pdma_model_dma::Dma<M>) -> TestResult {
    let base = ChannelBlock::ALIGNMENT;
    for (offset, value) in [
        (
            base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
            0x1000_u64,
        ),
        (
            base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
            0x1100,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            base + ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            base + ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
            2,
        ),
        (
            base + ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
            1,
        ),
    ] {
        device.write(offset, &value.to_le_bytes())?;
    }
    Ok(())
}
