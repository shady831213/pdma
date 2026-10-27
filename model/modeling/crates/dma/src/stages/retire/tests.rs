use pdma_model_contract::dma::{
    ChannelBlock, ChannelConfiguration, ChannelId, ChannelRegisters, Command, CommandRegisters,
    CopyCommandDestinationValues, CopyCommandSourceValues, EventRegisters, LinearEndpoint,
};

use crate::{Dma, Error, PrepareChunk, ReadCopy, WriteCopy};
mod collection;
mod identity;
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_pipeline::{Operation, StepOutcome, Then};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn queued_channel(channel: ChannelId) -> TestResult<Dma<SharedMemory>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        0x1000,
        MemoryAlignment::ONE,
    )?])?;
    let command = Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 17,
        copy_length: 4,
        sg_context: 0,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0x1200,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0x1300,
            offset: 0,
        }),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    memory.write(0x1000.into(), MemoryAlignment::ONE, &command.to_bytes()?)?;
    memory.write(0x1200.into(), MemoryAlignment::ONE, &[1, 3, 5, 7])?;
    let mut dma = Dma::new(memory, 2)?;
    configure_channel(&mut dma, channel)?;
    Ok(dma)
}

fn configure_channel(dma: &mut Dma<SharedMemory>, channel: ChannelId) -> TestResult {
    let base = channel.register_offset(2).ok_or("channel")?;
    for (offset, address) in [
        (ChannelBlock::COMMAND.offset(), 0x1000_u64),
        (ChannelBlock::EVENT.offset(), 0x1100),
    ] {
        dma.write(base + offset, &address.to_le_bytes())?;
    }
    for (offset, value) in [
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
            2_u32,
        ),
        (
            ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
            2,
        ),
        (
            ChannelBlock::CONTROL.offset()
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::ENABLE.offset(),
            1,
        ),
        (
            ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
            1,
        ),
    ] {
        dma.write(base + offset, &value.to_le_bytes())?;
    }
    Ok(())
}

#[test]
fn wrong_report_owner_cannot_stop_or_publish_into_a_channel() -> TestResult {
    let mut dma = queued_channel(ChannelId::new(1))?;
    let request = dma.command_request(ChannelId::new(1))?;
    let StepOutcome::Complete(Ok(fetched)) =
        Operation::new(request).step(&mut crate::Fetch, &mut dma)?
    else {
        return Err("fetch required".into());
    };
    let work = fetched.work.ok_or("owner")?;
    let event = pdma_model_contract::dma::Event {
        id: 17,
        code: pdma_model_contract::dma::EventCode::ReadError,
        info: pdma_model_contract::dma::EventInfoValues::ReadError(
            pdma_model_contract::dma::ErrorInfo { bytes_completed: 0 },
        ),
    };
    let channel = dma.channels.first_mut().ok_or("channel")?;
    assert_eq!(channel.report(work, &event, &dma.memory), Error::Routing);
    assert!(!channel.halted());
    assert_eq!(channel.event.pointers().producer, 0);
    Ok(())
}

#[test]
fn raw_error_after_consuming_flow_keeps_residency_until_tail_cleanup() -> TestResult {
    use crate::{ExecutionContext, Flow};
    use hardware_interface_pipeline::{RunError, Stage, StepResult};
    use std::convert::Infallible;

    struct RawTail {
        pending: Option<Flow<crate::Completion>>,
        called: bool,
    }
    impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for RawTail {
        type Input = Flow<crate::Completion>;
        type Output = Flow<crate::Completion>;
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
    let pipeline = crate::CommandExecution::new(Then::new(
        Then::new(PrepareChunk::default(), ReadCopy::default()),
        WriteCopy::default(),
    ));
    let mut hardware = dma.hardware_with(Then::new(
        pipeline,
        RawTail {
            pending: None,
            called: false,
        },
    ));
    let mut operation = Operation::new(());
    let mut raw_error = false;
    // When: the post-execution tail returns a raw error after consuming the Flow.
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
            .event
            .pointers()
            .producer,
        0
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
