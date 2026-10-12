use crate::{
    RESET,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{Operation, RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{ChannelBlock, Command, CommandRegisters};
use pdma_model_dma::{ChannelFatal, ChannelTask, Dma, Error, Intake, ResetControl, RunCommand};
use std::{cell::Cell, ops::ControlFlow, rc::Rc};

struct DropWitness(Rc<Cell<usize>>);

impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Retained {
    intake: Operation<Intake, Dma<SharedMemory>>,
    task: Option<ChannelTask<Command>>,
    witness: Option<DropWitness>,
}

struct RetainingChild {
    intake: Intake,
    drops: Rc<Cell<usize>>,
    calls: Rc<Cell<usize>>,
    terminal: StepResult<(), Error>,
}

impl Stage<Dma<SharedMemory>> for RetainingChild {
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = Retained;

    fn start((): ()) -> Retained {
        Retained {
            intake: Operation::new(()),
            task: None,
            witness: None,
        }
    }

    fn step(
        &mut self,
        device: &mut Dma<SharedMemory>,
        state: &mut Retained,
    ) -> StepResult<(), Error> {
        self.calls.set(self.calls.get() + 1);
        if state.task.is_some() {
            return std::mem::replace(&mut self.terminal, Ok(StepOutcome::InProgress));
        }
        Ok(match state.intake.step(&mut self.intake, device)? {
            StepOutcome::Complete(Ok(task)) => {
                state.task = Some(task);
                state.witness = Some(DropWitness(Rc::clone(&self.drops)));
                StepOutcome::InProgress
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::InProgress => StepOutcome::InProgress,
        })
    }
}

#[test]
fn every_terminal_outcome_disposes_child_state_before_reset_acknowledgement() -> TestResult {
    for terminal in [
        Ok(StepOutcome::Complete(Ok(()))),
        Ok(StepOutcome::Complete(Err(Error::Reset))),
        Ok(StepOutcome::Complete(Err(Error::ChannelFatal(
            ChannelFatal::ResponseWrite,
        )))),
        Ok(StepOutcome::Complete(Err(Error::Halted))),
        Ok(StepOutcome::Complete(Err(Error::Routing))),
        Err(RunError::Exhausted),
    ] {
        // Given: a child retains accepted work until the lifecycle wrapper terminates.
        let expected = match terminal {
            Ok(StepOutcome::Complete(
                Ok(()) | Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted),
            )) => Ok(StepOutcome::Complete(Ok(ControlFlow::Continue(())))),
            Ok(StepOutcome::Complete(Err(error))) => Ok(StepOutcome::Complete(Err(error))),
            Err(error) => Err(error),
            Ok(StepOutcome::InProgress) => return Err("nonterminal fixture".into()),
        };
        let (mut device, memory) = setup(0x1100, 2)?;
        submit(&mut device, &memory, command())?;
        let drops = Rc::new(Cell::new(0));
        let calls = Rc::new(Cell::new(0));
        let mut hardware = RunCommand::new(RetainingChild {
            intake: device.intake(),
            drops: Rc::clone(&drops),
            calls: Rc::clone(&calls),
            terminal,
        });
        let mut state = <RunCommand<RetainingChild> as Stage<Dma<SharedMemory>>>::start(());
        let mut accepted = false;
        for _ in 0..32 {
            let _ = hardware.step(&mut device, &mut state)?;
            if state.as_ref().is_some_and(|child| child.task.is_some()) {
                accepted = true;
                break;
            }
        }
        assert!(accepted);
        assert_eq!(drops.get(), 0);
        device.write(RESET, &1_u32.to_le_bytes())?;
        let _ = ResetControl.step(&mut device, &mut ())?;
        assert_eq!(read_word(&mut device, RESET)?, 1);
        // When: the lifecycle wrapper observes the terminal child result.
        let outcome = hardware.step(&mut device, &mut state);
        // Then: state is disposed while the caller still retains the wrapper state.
        assert_eq!(outcome, expected);
        assert_eq!(drops.get(), 1);
        assert_eq!(read_word(&mut device, RESET)?, 1);
        let _ = ResetControl.step(&mut device, &mut ())?;
        assert_eq!(read_word(&mut device, RESET)?, 0);
        let terminal_calls = calls.get();
        assert_eq!(
            hardware.step(&mut device, &mut state),
            Err(RunError::Exhausted)
        );
        assert_eq!(calls.get(), terminal_calls);
        assert_eq!(drops.get(), 1);
    }
    Ok(())
}

#[test]
fn completed_state_cannot_retire_a_newer_accepted_command() -> TestResult {
    // Given: an old completed state and a newer command accepted by the same hardware.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = RunCommand::new(RetainingChild {
        intake: device.intake(),
        drops: Rc::new(Cell::new(0)),
        calls: Rc::new(Cell::new(0)),
        terminal: Ok(StepOutcome::Complete(Ok(()))),
    });
    let mut old = <RunCommand<RetainingChild> as Stage<Dma<SharedMemory>>>::start(());
    let mut completed = false;
    for _ in 0..32 {
        if matches!(
            hardware.step(&mut device, &mut old)?,
            StepOutcome::Complete(Ok(_))
        ) {
            completed = true;
            break;
        }
    }
    assert!(completed);
    memory.write(
        MemoryAddress::new(0x1000 + u64::try_from(Command::RECORD_SIZE)?),
        MemoryAlignment::ONE,
        &command().to_bytes()?,
    )?;
    device.write(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut newer = RunCommand::new(RetainingChild {
        intake: device.intake(),
        drops: Rc::new(Cell::new(0)),
        calls: Rc::new(Cell::new(0)),
        terminal: Ok(StepOutcome::Complete(Err(Error::Reset))),
    });
    let mut current = <RunCommand<RetainingChild> as Stage<Dma<SharedMemory>>>::start(());
    let mut accepted = false;
    for _ in 0..32 {
        let _ = newer.step(&mut device, &mut current)?;
        if current.as_ref().is_some_and(|child| child.task.is_some()) {
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    device.write(RESET, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, RESET)?, 1);
    // When: the caller steps the old completed state again.
    assert_eq!(
        hardware.step(&mut device, &mut old),
        Err(RunError::Exhausted)
    );
    let _ = ResetControl.step(&mut device, &mut ())?;
    // Then: reset remains busy until the current owner retires.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    assert_eq!(
        newer.step(&mut device, &mut current)?,
        StepOutcome::Complete(Ok(ControlFlow::Continue(())))
    );
    let _ = ResetControl.step(&mut device, &mut ())?;
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}
