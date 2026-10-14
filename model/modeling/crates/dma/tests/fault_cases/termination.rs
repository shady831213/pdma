use super::*;
use hardware_interface_pipeline::{RunError, Stage, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{Error, Pipeline, ResetControl, Service, Work};

struct PeerFailure {
    pipeline: Pipeline,
    raw_error: bool,
}

struct Invocation {
    child: <Pipeline as Stage<Dma<Paged>>>::State,
    work: Option<Work>,
}

impl Stage<Dma<Paged>> for PeerFailure {
    type Input = Option<Work>;
    type Output = ();
    type Error = Error;
    type State = Invocation;

    fn start(work: Self::Input) -> Self::State {
        Invocation {
            child: <Pipeline as Stage<Dma<Paged>>>::start(work),
            work,
        }
    }

    fn step(&mut self, device: &mut Dma<Paged>, state: &mut Invocation) -> StepResult<(), Error> {
        if state
            .work
            .is_some_and(|work| work.channel() == ChannelId::new(1))
        {
            if self.raw_error {
                Err(RunError::Exhausted)
            } else {
                Ok(StepOutcome::Complete(Err(Error::Routing)))
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
        let pipeline = crate::support::single_chunk(&device).downstream;
        let mut engine = device.service_with(PeerFailure {
            pipeline,
            raw_error,
        });
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
