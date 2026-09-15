use crate::{
    RESET,
    support::{TestResult, command, read_word, setup, submit},
};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::{ChannelBlock, Command, CommandRegisters, DeviceRegisters};
use pdma_model_dma::{ChannelTask, Device, Error, Fault, Intake, ResetControl, RunCommand};
use std::{cell::Cell, ops::ControlFlow, rc::Rc};

struct DropWitness(Rc<Cell<usize>>);

impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Retained {
    intake: Operation<Intake, Device<SharedMemory>>,
    task: Option<ChannelTask<Command>>,
    witness: Option<DropWitness>,
}

struct RetainingChild {
    intake: Intake,
    drops: Rc<Cell<usize>>,
    calls: Rc<Cell<usize>>,
    terminal: StepOutcome<(), Error, ()>,
}

impl Stage<Device<SharedMemory>> for RetainingChild {
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
        device: &mut Device<SharedMemory>,
        state: &mut Retained,
    ) -> StepOutcome<(), Error, ()> {
        self.calls.set(self.calls.get() + 1);
        if state.task.is_some() {
            return std::mem::replace(&mut self.terminal, StepOutcome::Pending(()));
        }
        match state.intake.step(&mut self.intake, device) {
            StepOutcome::Complete(Ok(task)) => {
                state.task = Some(task);
                state.witness = Some(DropWitness(Rc::clone(&self.drops)));
                StepOutcome::Pending(())
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(()) => StepOutcome::Pending(()),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}

#[test]
fn every_terminal_outcome_disposes_child_state_before_reset_acknowledgement() -> TestResult {
    for terminal in [
        StepOutcome::Complete(Ok(())),
        StepOutcome::Complete(Err(Error::Reset)),
        StepOutcome::Complete(Err(Error::Fault(Fault::ResponseWrite))),
        StepOutcome::Complete(Err(Error::Halted)),
        StepOutcome::Complete(Err(Error::Routing)),
        StepOutcome::Exhausted,
    ] {
        // Given: a child that retains accepted work even when returning a terminal result.
        let expected = match terminal {
            StepOutcome::Complete(Ok(()) | Err(Error::Reset | Error::Fault(_) | Error::Halted)) => {
                StepOutcome::Complete(Ok(ControlFlow::Continue(())))
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
            StepOutcome::Progressed | StepOutcome::Pending(()) => {
                return Err("nonterminal fixture".into());
            }
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
        let mut state = <RunCommand<RetainingChild> as Stage<Device<SharedMemory>>>::start(());
        let mut accepted = false;
        for _ in 0..32 {
            if matches!(
                hardware.step(&mut device, &mut state),
                StepOutcome::Pending(())
            ) {
                accepted = true;
                break;
            }
        }
        assert!(accepted);
        assert_eq!(drops.get(), 0);
        device.write(RESET, &1_u32.to_le_bytes())?;
        let _ = ResetControl.step(&mut device, &mut ());
        assert_eq!(read_word(&mut device, RESET)?, 1);
        // When: the lifecycle wrapper observes the terminal child result.
        let outcome = hardware.step(&mut device, &mut state);
        // Then: state is disposed while the caller still retains the wrapper state.
        assert_eq!(outcome, expected);
        assert_eq!(drops.get(), 1);
        assert_eq!(read_word(&mut device, RESET)?, 1);
        let _ = ResetControl.step(&mut device, &mut ());
        assert_eq!(read_word(&mut device, RESET)?, 0);
        let terminal_calls = calls.get();
        assert_eq!(
            hardware.step(&mut device, &mut state),
            StepOutcome::Exhausted
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
        terminal: StepOutcome::Complete(Ok(())),
    });
    let mut old = <RunCommand<RetainingChild> as Stage<Device<SharedMemory>>>::start(());
    let mut completed = false;
    for _ in 0..32 {
        if matches!(
            hardware.step(&mut device, &mut old),
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
        DeviceRegisters::CHANNELS_OFFSET
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut newer = RunCommand::new(RetainingChild {
        intake: device.intake(),
        drops: Rc::new(Cell::new(0)),
        calls: Rc::new(Cell::new(0)),
        terminal: StepOutcome::Complete(Err(Error::Reset)),
    });
    let mut current = <RunCommand<RetainingChild> as Stage<Device<SharedMemory>>>::start(());
    let mut accepted = false;
    for _ in 0..32 {
        if matches!(
            newer.step(&mut device, &mut current),
            StepOutcome::Pending(())
        ) {
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    device.write(RESET, &1_u32.to_le_bytes())?;
    let _ = ResetControl.step(&mut device, &mut ());
    assert_eq!(read_word(&mut device, RESET)?, 1);
    // When: the caller steps the old completed state again.
    assert_eq!(hardware.step(&mut device, &mut old), StepOutcome::Exhausted);
    let _ = ResetControl.step(&mut device, &mut ());
    // Then: reset remains busy until the current owner retires.
    assert_eq!(read_word(&mut device, RESET)?, 1);
    assert_eq!(
        newer.step(&mut device, &mut current),
        StepOutcome::Complete(Ok(ControlFlow::Continue(())))
    );
    let _ = ResetControl.step(&mut device, &mut ());
    assert_eq!(read_word(&mut device, RESET)?, 0);
    Ok(())
}
