use super::*;
mod lifecycle;
use hardware_interface_pipeline::{AfterEach, AlreadyComplete, Repeat, Stage, StepOutcome, Then};
use pdma_model_dma::{
    ChannelTask, CheckRanges, CommandTurn, Error, NextChunk, Pipeline, ReadCopy, Resettable,
    RunCommand, UpdateIrq, WriteCopy,
};
use std::{cell::Cell, rc::Rc};

struct Witness {
    drops: Rc<Cell<u32>>,
    identity: Box<u64>,
}

impl Drop for Witness {
    fn drop(&mut self) {
        assert_eq!(*self.identity, 91);
        self.drops.set(self.drops.get() + 1);
    }
}

struct State {
    child: Operation<Pipeline, Dma<Paged>>,
    witness: Option<Witness>,
}

struct Enclosing {
    pipeline: Pipeline,
    drops: Rc<Cell<u32>>,
    starts: Rc<Cell<u32>>,
}

impl Enclosing {
    fn new(dma: &Dma<Paged>, drops: Rc<Cell<u32>>, starts: Rc<Cell<u32>>) -> Self {
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckRanges, Repeat::new(chunk));
        Self {
            pipeline: Then::new(
                Resettable::new(AfterEach::new(execution, UpdateIrq)),
                dma.responses(),
            ),
            drops,
            starts,
        }
    }
}

impl Stage<Dma<Paged>> for Enclosing {
    type Input = ChannelTask<Command>;
    type Output = ();
    type Error = Error;
    type Wait = ();
    type State = State;

    fn start(task: Self::Input) -> State {
        State {
            child: Operation::new(task),
            witness: None,
        }
    }

    fn step(
        &mut self,
        dma: &mut Dma<Paged>,
        mut state: State,
    ) -> StepOutcome<State, (), Error, ()> {
        if state.witness.is_none() {
            self.starts.set(self.starts.get() + 1);
            state.witness = Some(Witness {
                drops: Rc::clone(&self.drops),
                identity: Box::new(91),
            });
        }
        match state.child.step(&mut self.pipeline, dma) {
            Ok(RunOutcome::Complete(result)) => StepOutcome::Complete(result),
            Ok(RunOutcome::Progressed) => StepOutcome::Progressed(state),
            Ok(RunOutcome::Pending(())) => StepOutcome::Pending(state, ()),
            Err(AlreadyComplete) => StepOutcome::Complete(Err(Error::Routing)),
        }
    }
}

#[test]
fn direct_suspension_transfers_non_clone_enclosing_state_until_final_retirement() -> TestResult {
    // Given: arbitrary non-Clone state encloses the real command composition.
    let mut dma = setup()?.dma;
    let command = streams(&dma)?;
    dma.memory.missing.set(Some((0x1343, true)));
    submit(&mut dma, command, 0)?;
    let drops = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(0));
    let pipeline = Then::new(
        dma.intake(),
        Enclosing::new(&dma, Rc::clone(&drops), Rc::clone(&starts)),
    );
    let mut engine = RunCommand::new(pipeline);
    let mut operation = Operation::new(());
    let mut suspended = None;
    for _ in 0..256 {
        if let RunOutcome::Complete(result) = operation.step(&mut engine, &mut dma)? {
            match result? {
                CommandTurn::Finished => return Err("faulted command retired".into()),
                CommandTurn::Suspended(state) => suspended = Some(state),
            }
            break;
        }
    }
    assert!(operation.is_complete());
    assert_eq!(drops.get(), 0);
    assert_eq!(starts.get(), 1);
    assert!(matches!(
        operation.step(&mut engine, &mut dma),
        Err(AlreadyComplete)
    ));
    // When: repair resumes the transferred state after changing the original source.
    dma.memory.missing.set(None);
    dma.memory
        .memory
        .write(0x1240.into(), MemoryAlignment::ONE, b"12345")?;
    resume(&mut dma, 1)?;
    let mut resumed = Operation::from_state(suspended.ok_or("missing suspension")?);
    for _ in 0..256 {
        if let RunOutcome::Complete(result) = resumed.step(&mut engine, &mut dma)? {
            assert!(matches!(result?, CommandTurn::Finished));
            break;
        }
    }
    // Then: enclosing state was never rebuilt; cached bytes publish once and drop once.
    assert!(resumed.is_complete());
    assert_eq!(starts.get(), 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(
        dma.memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        *b"cdefgh"
    );
    assert_eq!(response(&dma)?.bytes_completed(), command.copy_length);
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        1
    );
    Ok(())
}

#[test]
fn channel_bank_preserves_non_clone_enclosing_state_across_repeated_faults() -> TestResult {
    // Given: the custom enclosing state is owned by a complete command-loop graph.
    let mut dma = setup()?.dma;
    let command = streams(&dma)?;
    dma.memory.missing.set(Some((0x1343, true)));
    submit(&mut dma, command, 0)?;
    let drops = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(0));
    let mut engine =
        dma.command_loop_with(Enclosing::new(&dma, Rc::clone(&drops), Rc::clone(&starts)));
    let mut operation = Operation::new(());
    // When: the bank restores the same command across two faults before repair.
    for expected_faults in 1..=2 {
        for _ in 0..256 {
            assert!(matches!(
                operation.step(&mut engine, &mut dma)?,
                RunOutcome::Progressed | RunOutcome::Pending(())
            ));
        }
        assert_eq!(dma.memory.faults.get(), expected_faults);
        assert_eq!(drops.get(), 0);
        assert_eq!(starts.get(), 1);
        resume(&mut dma, 1)?;
    }
    dma.memory.missing.set(None);
    dma.memory
        .memory
        .write(0x1240.into(), MemoryAlignment::ONE, b"12345")?;
    for _ in 0..256 {
        assert!(matches!(
            operation.step(&mut engine, &mut dma)?,
            RunOutcome::Progressed | RunOutcome::Pending(())
        ));
    }
    // Then: the exact enclosing state and successful read scratch survived every turn.
    assert_eq!(starts.get(), 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(
        dma.memory
            .memory
            .read_typed::<[u8; 6]>(0x1340.into(), MemoryAlignment::ONE)?,
        *b"cdefgh"
    );
    assert_eq!(
        read_word(&mut dma, RESP + ResponseRegisters::PRODUCER.offset())?,
        1
    );
    Ok(())
}
