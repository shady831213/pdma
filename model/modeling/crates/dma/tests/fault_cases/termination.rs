use super::*;
use hardware_interface_pipeline::{RunError, Stage, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{
    ChunkInput, Error, ExecutionContext, ExecutionError, ReadPipeline, ResetControl, Service,
};

struct PeerFailure {
    pipeline: ReadPipeline,
    raw_error: bool,
}

struct Invocation {
    child: <ReadPipeline as Stage<ExecutionContext<'static, Paged>>>::State,
    channel: Option<ChannelId>,
}

impl Stage<ExecutionContext<'_, Paged>> for PeerFailure {
    type Input = Option<ChunkInput>;
    type Output = Option<pdma_model_dma::Payload>;
    type Error = ExecutionError;
    type State = Invocation;

    fn start(work: Self::Input) -> Self::State {
        let channel = work.as_ref().map(ChunkInput::channel);
        Invocation {
            child: <ReadPipeline as Stage<ExecutionContext<'static, Paged>>>::start(work),
            channel,
        }
    }

    fn step(
        &mut self,
        device: &mut ExecutionContext<'_, Paged>,
        state: &mut Invocation,
    ) -> StepResult<Self::Output, ExecutionError> {
        if state
            .channel
            .is_some_and(|channel| channel == ChannelId::new(1))
        {
            if self.raw_error {
                Err(RunError::Exhausted)
            } else {
                Ok(StepOutcome::Complete(Err(Error::Routing.into())))
            }
        } else {
            self.pipeline.step(device, &mut state.child)
        }
    }
}

#[test]
fn peer_pipeline_termination_preserves_parked_hardware_until_reset() -> TestResult {
    for raw_error in [false, true] {
        // Given: one faulted channel and a peer whose custom pipeline will terminate.
        let (mut device, _, _) = super::peers::pair()?;
        let pipeline = hardware_interface_pipeline::Then::new(
            pdma_model_dma::PrepareChunk,
            pdma_model_dma::ReadCopy,
        );
        let mut engine = device.service_with(hardware_interface_pipeline::Then::new(
            PeerFailure {
                pipeline,
                raw_error,
            },
            hardware_interface_pipeline::Then::new(pdma_model_dma::WriteCopy, device.responses()),
        ));
        let mut state = <Service<PeerFailure> as Stage<Dma<Paged>>>::start(());
        let mut terminal = None;
        for _ in 0..512 {
            match engine.step(&mut device, &mut state) {
                Ok(StepOutcome::InProgress) => {}
                Ok(StepOutcome::Complete(Ok(()))) => {
                    state = <Service<PeerFailure> as Stage<Dma<Paged>>>::start(());
                }
                result => {
                    terminal = Some(result);
                    break;
                }
            }
        }
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(device.memory.faults.get(), 1);
        // When: the peer terminates and a fresh service processes explicit channel resets.
        let expected = if raw_error {
            Err(RunError::Exhausted)
        } else {
            Ok(StepOutcome::Complete(Err(Error::Routing)))
        };
        assert_eq!(terminal, Some(expected));
        assert_eq!(
            engine.step(&mut device, &mut state),
            Err(RunError::Exhausted)
        );
        for index in 0..2 {
            let reset = ChannelId::new(index).admin_offset(2).ok_or("reset")?;
            device.write(reset, &1_u32.to_le_bytes())?;
        }
        let _ = ResetControl.step(&mut device, &mut ())?;
        let mut cleanup = device.service();
        let mut operation = Operation::new(());
        for _ in 0..128 {
            if let StepOutcome::Complete(result) = operation.step(&mut cleanup, &mut device)? {
                result?;
                break;
            }
        }
        // Then: hardware cleanup completes independently of the retained failed invocation state.
        for index in 0..2 {
            assert_eq!(
                read_word(
                    &mut device,
                    ChannelId::new(index).admin_offset(2).ok_or("reset")?
                )?,
                0
            );
        }
        assert_eq!(fault_status(&mut device)?, 0);
        assert_eq!(device.memory.faults.get(), 1);
        assert_eq!(
            read_word(&mut device, RESP + ResponseRegisters::PRODUCER.offset())?,
            0
        );
    }
    Ok(())
}
