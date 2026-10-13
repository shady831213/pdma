use super::*;
use hardware_interface_pipeline::{AfterEach, Repeat, RunError, Stage, StepResult, Then};
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::{
    ChannelTask, CheckRanges, CommandLoop, Error, NextChunk, Pipeline, ReadCopy, ResetControl,
    Resettable, UpdateIrq, WriteCopy,
};
use std::{cell::Cell, rc::Rc};

struct Witness(Rc<Cell<usize>>);

impl Drop for Witness {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Witnessed {
    pipeline: Pipeline,
    drops: Rc<Cell<usize>>,
    raw_error: bool,
}

struct Retained {
    child: <Pipeline as Stage<Dma<Paged>>>::State,
    channel: ChannelId,
    witness: Option<Witness>,
}

impl Stage<Dma<Paged>> for Witnessed {
    type Input = ChannelTask<Command>;
    type Output = ();
    type Error = Error;
    type State = Retained;

    fn start(input: Self::Input) -> Self::State {
        let channel = input.channel();
        Retained {
            child: <Pipeline as Stage<Dma<Paged>>>::start(input),
            channel,
            witness: None,
        }
    }

    fn step(&mut self, device: &mut Dma<Paged>, state: &mut Retained) -> StepResult<(), Error> {
        state
            .witness
            .get_or_insert_with(|| Witness(Rc::clone(&self.drops)));
        if state.channel == ChannelId::new(1) {
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
fn root_termination_drops_parked_continuations_before_reset_can_complete() -> TestResult {
    for raw_error in [false, true] {
        // Given: channel zero parks real payload work while a peer will terminate the graph.
        let (mut device, _, _) = super::peers::pair()?;
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckRanges, Repeat::new(chunk));
        let pipeline = Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            device.responses(),
        );
        let drops = Rc::new(Cell::new(0));
        let mut engine = device.command_loop_with(Witnessed {
            pipeline,
            drops: Rc::clone(&drops),
            raw_error,
        });
        let mut state = <CommandLoop<Witnessed> as Stage<Dma<Paged>>>::start(());
        for _ in 0..256 {
            assert!(matches!(
                engine.step(&mut device, &mut state)?,
                StepOutcome::InProgress
            ));
            if fault_status(&mut device)? == 1 {
                break;
            }
        }
        assert_eq!(fault_status(&mut device)?, 1);
        assert_eq!(drops.get(), 0);
        // When: the accepted peer returns an unscoped error with the raw root state still retained.
        let mut terminal = None;
        for _ in 0..128 {
            match engine.step(&mut device, &mut state) {
                Ok(StepOutcome::InProgress) => {}
                result => {
                    terminal = Some(result);
                    break;
                }
            }
        }
        let expected = if raw_error {
            Err(RunError::Exhausted)
        } else {
            Ok(StepOutcome::Complete(Err(Error::Routing)))
        };
        assert_eq!(terminal, Some(expected));
        // Then: both invocation states are dropped, and no retained reference delays reset.
        assert_eq!(drops.get(), 2);
        for index in 0..2 {
            let reset = ChannelId::new(index).admin_offset(2).ok_or("reset")?;
            device.write(reset, &1_u32.to_le_bytes())?;
            let _ = ResetControl.step(&mut device, &mut ())?;
            assert_eq!(read_word(&mut device, reset)?, 0);
        }
        assert_eq!(
            engine.step(&mut device, &mut state),
            Err(RunError::Exhausted)
        );
        assert_eq!(drops.get(), 2);
    }
    Ok(())
}
