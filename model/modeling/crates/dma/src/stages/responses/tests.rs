use crate::{CompletionInput, Dma, Error, PrepareChunk, ReadCopy, WriteCopy};
mod identity;
use hardware_interface_pipeline::{Operation, StepOutcome, Then};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, CompletionFlagsValues,
    Endpoint, ResponseRegisters,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn queued_channel(channel: ChannelId) -> TestResult<Dma<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let command = Command {
        id: 17,
        copy_length: 4,
        sg_context: 0,
        flags: CompletionFlagsValues { ioc: false },
        source: Endpoint::linear(0x1200, 0),
        destination: Endpoint::linear(0x1300, 0),
    };
    memory.write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
    memory.write(0x1200.into(), MemoryAlignment::ONE, &[1, 3, 5, 7])?;
    let mut dma = Dma::new(memory, 2)?;
    let base = channel.register_offset(2).ok_or("channel")?;
    for (offset, address) in [
        (ChannelBlock::COMMAND.offset(), 0x1000_u64),
        (ChannelBlock::RESPONSE.offset(), 0x1100),
    ] {
        dma.write(base + offset, &address.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
            1,
        ),
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
            1,
        ),
    ] {
        dma.write(base + offset, &value.to_le_bytes())?;
    }
    Ok(dma)
}

#[test]
fn cross_instance_response_port_mismatch_retires_its_original_owner() -> TestResult {
    // Given: publicly composed response ports from a smaller instance.
    let mut dma = queued_channel(ChannelId::new(1))?;
    let smaller = Dma::new(SharedMemory::new(), 1)?;
    let mut hardware = dma.hardware_with(Then::new(
        Then::new(PrepareChunk::default(), ReadCopy::default()),
        Then::new(
            WriteCopy::default(),
            Then::new(CompletionInput, smaller.responses()),
        ),
    ));
    let mut operation = Operation::new(());
    let mut outcome = None;
    // When: channel one's completion reaches the one-port response fan.
    for _ in 0..256 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
            outcome = Some(result);
            break;
        }
    }
    // Then: routing failure reaches retirement and reset can finish.
    assert_eq!(outcome, Some(Err(Error::Routing)));
    assert!(
        dma.channels
            .iter()
            .all(|channel| channel.resident.is_none())
    );
    dma.request_reset();
    assert!(!dma.resetting());
    Ok(())
}

#[test]
fn raw_error_after_consuming_flow_keeps_residency_until_tail_cleanup() -> TestResult {
    use crate::{ExecutionContext, Flow};
    use hardware_interface_pipeline::{RunError, Stage, StepResult};
    use std::convert::Infallible;

    struct RawTail {
        pending: Option<Flow<()>>,
        called: bool,
    }
    impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for RawTail {
        type Input = Flow<()>;
        type Output = Flow<()>;
        type Error = Infallible;
        fn step(
            &mut self,
            context: &mut ExecutionContext<'_, M, SB>,
            input: &mut Option<Self::Input>,
        ) -> StepResult<Self::Output, Infallible> {
            if self.pending.is_none() {
                self.pending = input.take();
            }
            let Some(pending) = self.pending.as_ref() else {
                return Ok(StepOutcome::InProgress);
            };
            if !self.called {
                self.called = true;
                return Err(RunError::Exhausted);
            }
            let error = pending.lifecycle(context).err();
            let output = self.pending.take().ok_or(RunError::Exhausted)?;
            Ok(StepOutcome::Complete(Ok(match error {
                Some(error) => output.stop(error),
                None => output,
            })))
        }
    }
    // Given: an ordinary copy followed by a tail that consumes its owned result.
    let mut dma = queued_channel(ChannelId::new(1))?;
    let pipeline = Then::new(
        Then::new(PrepareChunk::default(), ReadCopy::default()),
        Then::new(
            WriteCopy::default(),
            Then::new(CompletionInput, dma.responses()),
        ),
    );
    let mut hardware = dma.hardware_with(Then::new(
        pipeline,
        RawTail {
            pending: None,
            called: false,
        },
    ));
    let mut operation = Operation::new(());
    let mut raw_error = false;
    // When: the post-publication tail returns a raw error after consuming the Flow.
    for _ in 0..256 {
        match operation.step(&mut hardware, &mut dma) {
            Err(RunError::Exhausted) => {
                raw_error = true;
                break;
            }
            result => {
                let _ = result?;
            }
        }
    }
    assert!(raw_error);
    assert!(!operation.is_complete());
    assert!(dma.channels.get(1).ok_or("channel")?.resident.is_some());
    assert_eq!(
        dma.channels
            .get(1)
            .ok_or("channel")?
            .response
            .pointers()
            .producer,
        1
    );
    dma.request_reset();
    assert!(dma.resetting());
    for _ in 0..128 {
        if let StepOutcome::Complete(result) = operation.step(&mut hardware, &mut dma)? {
            result?;
            break;
        }
    }
    // Then: the same tail resumes cleanup and retirement acknowledges reset.
    assert!(operation.is_complete());
    assert!(!dma.resetting());
    let mut bytes = [0; 4];
    dma.memory
        .read(0x1300.into(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [1, 3, 5, 7]);
    Ok(())
}
