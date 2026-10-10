use crate::{
    RESET,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{AlreadyComplete, Operation, RunOutcome, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::dma::{ChannelBlock, Command, CommandRegisters};
use pdma_model_dma::{
    ChannelFatal, ChannelTask, CommandTurn, Dma, Error, Intake, ResetControl, RunCommand,
};
use std::{cell::Cell, rc::Rc};

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
    terminal: Result<(), Error>,
}

impl Stage<Dma<SharedMemory>> for RetainingChild {
    type Input = ();
    type Output = ();
    type Error = Error;
    type Wait = ();
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
        mut state: Retained,
    ) -> StepOutcome<Retained, (), Error, ()> {
        self.calls.set(self.calls.get() + 1);
        if state.task.is_some() {
            return StepOutcome::Complete(self.terminal);
        }
        match state.intake.step(&mut self.intake, device) {
            Ok(RunOutcome::Complete(Ok(task))) => {
                state.task = Some(task);
                state.witness = Some(DropWitness(Rc::clone(&self.drops)));
                StepOutcome::Pending(state, ())
            }
            Ok(RunOutcome::Complete(Err(error))) => StepOutcome::Complete(Err(error)),
            Ok(RunOutcome::Progressed) => StepOutcome::Progressed(state),
            Ok(RunOutcome::Pending(())) => StepOutcome::Pending(state, ()),
            Err(AlreadyComplete) => StepOutcome::Complete(Err(Error::Routing)),
        }
    }
}

#[test]
fn every_terminal_outcome_disposes_child_state_before_reset_acknowledgement() -> TestResult {
    for terminal in [
        Ok(()),
        Err(Error::Reset),
        Err(Error::ChannelFatal(ChannelFatal::ResponseWrite)),
        Err(Error::Halted),
        Err(Error::Routing),
    ] {
        // Given: a child that retains accepted work even when returning a terminal result.
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
        let mut state = Operation::<RunCommand<RetainingChild>, Dma<SharedMemory>>::new(());
        let mut accepted = false;
        for _ in 0..32 {
            if matches!(
                state.step(&mut hardware, &mut device),
                Ok(RunOutcome::Pending(()))
            ) {
                accepted = true;
                break;
            }
        }
        assert!(accepted);
        assert_eq!(drops.get(), 0);
        device.write(RESET, &1_u32.to_le_bytes())?;
        let _ = ResetControl.step(&mut device, ());
        assert_eq!(read_word(&mut device, RESET)?, 1);
        // When: the lifecycle wrapper observes the terminal child result.
        let outcome = state.step(&mut hardware, &mut device);
        // Then: state is disposed while the caller still retains the wrapper state.
        match terminal {
            Ok(()) | Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted) => {
                assert!(matches!(
                    outcome,
                    Ok(RunOutcome::Complete(Ok(CommandTurn::Finished)))
                ));
            }
            Err(expected) => assert!(
                matches!(outcome, Ok(RunOutcome::Complete(Err(error))) if error == expected)
            ),
        }
        assert_eq!(drops.get(), 1);
        assert_eq!(read_word(&mut device, RESET)?, 1);
        let _ = ResetControl.step(&mut device, ());
        assert_eq!(read_word(&mut device, RESET)?, 0);
        let terminal_calls = calls.get();
        assert!(matches!(
            state.step(&mut hardware, &mut device),
            Err(AlreadyComplete)
        ));
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
        terminal: Ok(()),
    });
    let mut old = Operation::<RunCommand<RetainingChild>, Dma<SharedMemory>>::new(());
    let mut completed = false;
    for _ in 0..32 {
        if matches!(
            old.step(&mut hardware, &mut device),
            Ok(RunOutcome::Complete(Ok(_)))
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
        terminal: Err(Error::Reset),
    });
    let mut current = Operation::<RunCommand<RetainingChild>, Dma<SharedMemory>>::new(());
    let mut accepted = false;
    for _ in 0..32 {
        if matches!(
            current.step(&mut newer, &mut device),
            Ok(RunOutcome::Pending(()))
        ) {
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    device.write(RESET, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, ());
    assert_eq!(read_word(&mut device, RESET)?, 1);
    // When: the caller steps the old completed state again.
    assert!(matches!(
        old.step(&mut hardware, &mut device),
        Err(AlreadyComplete)
    ));
    let _ = ResetControl.step(&mut device, ());
    // Then: reset remains busy until the current owner retires.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    assert!(matches!(
        current.step(&mut newer, &mut device),
        Ok(RunOutcome::Complete(Ok(CommandTurn::Finished)))
    ));
    let _ = ResetControl.step(&mut device, ());
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}
