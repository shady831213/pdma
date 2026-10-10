mod acquire;
mod lifecycle;
mod turn;

use crate::{ChannelTask, Dma, Error, Pipeline, ResetControl, RoundRobin, RunCommand};
use acquire::{Acquire, SelectionPort};
use hardware_interface_pipeline::{
    AfterEach, Arbitrate, BranchError, FanInState, MapError, OnContinue, Operation, Repeat, Stage,
    StepOutcome, Then, ThenState,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};
use std::ops::ControlFlow;
use turn::Park;

type Selection = MapError<Arbitrate<SelectionPort, RoundRobin>, fn(BranchError<Error>) -> Error>;
type Graph<S> = AfterEach<
    Repeat<Then<Then<Selection, Acquire<S>>, Then<OnContinue<RunCommand<S>>, Park>>>,
    ResetControl,
>;
type TurnState<T> = ThenState<
    ThenState<FanInState<usize, (), ChannelId>, ChannelId>,
    ThenState<ControlFlow<(), T>, ControlFlow<(), T>>,
>;

/// Ordinary channel selection followed by acquisition, one execution turn and parking.
#[derive(Debug)]
pub struct CommandLoop<S = Pipeline> {
    graph: Graph<S>,
}

/// The graph continuation and one exact suspended continuation per channel.
#[derive(Debug)]
pub struct CommandLoopState<T> {
    graph: (TurnState<T>, ()),
    parked: Vec<Option<T>>,
}

struct Frame<'a, T, M, SB> {
    device: &'a mut Dma<M, SB>,
    parked: &'a mut [Option<T>],
}

impl<S> CommandLoop<S> {
    pub(crate) fn new(count: usize, pipeline: S) -> Self {
        let selection = MapError::new(
            Arbitrate::new(
                (0..count)
                    .map(|index| SelectionPort(ChannelId::new(index)))
                    .collect(),
                RoundRobin::default(),
            ),
            Error::from as fn(BranchError<Error>) -> Error,
        );
        Self {
            graph: AfterEach::new(
                Repeat::new(Then::new(
                    Then::new(selection, Acquire::new()),
                    Then::new(OnContinue::new(RunCommand::new(pipeline)), Park),
                )),
                ResetControl,
            ),
        }
    }
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Dma<M, SB>> for CommandLoop<S>
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error, Wait = ()>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type Wait = ();
    type State = CommandLoopState<Operation<S, Dma<M, SB>>>;

    fn start((): ()) -> Self::State {
        CommandLoopState {
            graph: <Graph<S> as Stage<Frame<'_, Operation<S, Dma<M, SB>>, M, SB>>>::start(()),
            parked: Vec::new(),
        }
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        mut state: Self::State,
    ) -> StepOutcome<Self::State, (), Error, ()> {
        if state.parked.is_empty() {
            if state
                .parked
                .try_reserve_exact(device.channels.len())
                .is_err()
            {
                return StepOutcome::Complete(Err(Error::OperationAllocation));
            }
            state.parked.resize_with(device.channels.len(), || None);
        }
        let mut frame = Frame {
            device,
            parked: &mut state.parked,
        };
        if lifecycle::cleanup(&mut frame) {
            let _ = ResetControl.step(&mut frame, ());
            return StepOutcome::Progressed(state);
        }
        match self.graph.step(&mut frame, state.graph) {
            StepOutcome::Complete(result) => StepOutcome::Complete(result),
            StepOutcome::Progressed(graph) => {
                state.graph = graph;
                StepOutcome::Progressed(state)
            }
            StepOutcome::Pending(graph, ()) => {
                state.graph = graph;
                StepOutcome::Pending(state, ())
            }
        }
    }
}
