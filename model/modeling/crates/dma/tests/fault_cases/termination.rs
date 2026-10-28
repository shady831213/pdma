use super::*;
use hw_model_pipeline::{RunError, Stage, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{ChunkInput, Error, ExecutionContext, Pipeline};

struct PeerFailure {
    pipeline: Pipeline,
    raw_error: bool,
}

impl Stage<ExecutionContext<'_, Paged>> for PeerFailure {
    type Input = pdma_model_dma::Flow<ChunkInput>;
    type Output = pdma_model_dma::Flow<pdma_model_dma::Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        device: &mut ExecutionContext<'_, Paged>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.raw_error
            && input
                .as_ref()
                .is_some_and(|flow| flow.lifecycle(device).is_err())
        {
            self.raw_error = false;
            return Ok(StepOutcome::Complete(Ok(input
                .take()
                .ok_or(hw_model_pipeline::RunError::Exhausted)?
                .stop(Error::Reset))));
        }
        if input
            .as_ref()
            .and_then(pdma_model_dma::Flow::data)
            .is_some_and(|work| work.channel() == ChannelId::new(1))
        {
            if self.raw_error {
                Err(RunError::Exhausted)
            } else {
                Ok(StepOutcome::Complete(Ok(input
                    .take()
                    .ok_or(hw_model_pipeline::RunError::Exhausted)?
                    .stop(Error::Routing))))
            }
        } else {
            self.pipeline.step(device, input)
        }
    }
}

#[test]
fn peer_pipeline_termination_preserves_parked_hardware_until_reset() -> TestResult {
    for raw_error in [false, true] {
        // Given: one faulted channel and a peer whose custom pipeline will terminate.
        let (mut device, _, _) = super::peers::pair()?;
        let pipeline = crate::support::chunk_pipeline();
        let mut engine = device.hardware_with(PeerFailure {
            pipeline,
            raw_error,
        });
        let mut state = Operation::new(());
        let mut terminal = None;
        for _ in 0..512 {
            match state.step(&mut engine, &mut device) {
                Ok(StepOutcome::InProgress) => {}
                Ok(StepOutcome::Complete(Ok(()))) => {
                    state = Operation::new(());
                }
                result => {
                    terminal = Some(result);
                    break;
                }
            }
        }
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(device.memory.faults.get(), 1);
        // When: the peer fails and software requests explicit channel resets.
        let expected = if raw_error {
            Err(RunError::Exhausted)
        } else {
            Ok(StepOutcome::Complete(Err(Error::Routing)))
        };
        assert_eq!(terminal, Some(expected));
        assert_eq!(
            state.step(&mut engine, &mut device),
            Err(RunError::Exhausted)
        );
        for index in 0..2 {
            let reset = ChannelId::new(index).admin_offset(2).ok_or("reset")?;
            device.write(reset, &1_u32.to_le_bytes())?;
        }
        if raw_error {
            assert!(!state.is_complete());
            for _ in 0..128 {
                if let StepOutcome::Complete(result) = state.step(&mut engine, &mut device)? {
                    assert_eq!(result, Ok(()));
                    break;
                }
            }
            assert!(state.is_complete());
        }
        let mut cleanup = device.hardware();
        let mut operation = hw_model_pipeline::HardwareDriver::new();
        for _ in 0..128 {
            let _ = operation.step(&mut cleanup, &mut device)?;
        }
        // Then: reset retires the original continuation and leaves parked peer effects unchanged.
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
            read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
            0
        );
    }
    Ok(())
}
