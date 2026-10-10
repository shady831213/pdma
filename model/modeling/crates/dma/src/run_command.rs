use crate::{Dma, Error, Pipeline};
use hardware_interface_pipeline::{AlreadyComplete, Operation, RunOutcome, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;

/// Successful execution-turn termination, retaining an exact suspended continuation.
#[derive(Debug)]
pub enum CommandTurn<T> {
    /// The command retired and released its accepted references.
    Finished,
    /// The command yielded the engine without retiring its accepted references.
    Suspended(T),
}

/// One shared command engine, ending each turn on retirement or payload suspension.
#[derive(Debug)]
pub struct RunCommand<S = Pipeline> {
    pipeline: S,
}

impl<S> RunCommand<S> {
    /// Owns the downstream composition without creating request state.
    pub const fn new(pipeline: S) -> Self {
        Self { pipeline }
    }
}

impl<M: MemoryAccess<SB>, S, SB> Stage<Dma<M, SB>> for RunCommand<S>
where
    S: Stage<Dma<M, SB>, Output = (), Error = Error, Wait = ()>,
{
    type Input = S::Input;
    type Output = CommandTurn<Operation<S, Dma<M, SB>>>;
    type Error = Error;
    type Wait = ();
    type State = Operation<S, Dma<M, SB>>;

    fn start(input: Self::Input) -> Self::State {
        Operation::new(input)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        mut state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Error, ()> {
        let outcome = match state.step(&mut self.pipeline, device) {
            Ok(outcome) => outcome,
            Err(AlreadyComplete) => return StepOutcome::Complete(Err(Error::Routing)),
        };
        if matches!(outcome, RunOutcome::Complete(_)) {
            device.active = None;
        } else if device.active.as_ref().is_some_and(|task| {
            device
                .channels
                .get(task.channel.index())
                .is_some_and(|channel| channel.registers.values.fault_status.faulted())
        }) {
            return StepOutcome::Complete(Ok(CommandTurn::Suspended(state)));
        }
        device.refresh_maintenance();
        match outcome {
            RunOutcome::Complete(
                Ok(()) | Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted),
            ) => StepOutcome::Complete(Ok(CommandTurn::Finished)),
            RunOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            RunOutcome::Progressed => StepOutcome::Progressed(state),
            RunOutcome::Pending(()) => StepOutcome::Pending(state, ()),
        }
    }
}
