mod admission;
mod selected;

use super::{CheckRanges, Commands, Fetch};
use crate::{Dma, Error, Pipeline};
pub use admission::Grant;
use admission::Install;
use hardware_interface_pipeline::{MapError, RunError, Stage, StepOutcome, StepResult, Then};
use selected::Selected;

type Graph<S> = Then<
    MapError<Commands, fn(hardware_interface_pipeline::BranchError<Error>) -> Error>,
    Then<Grant, Selected<Then<Fetch, Then<CheckRanges, Then<Install, S>>>>>,
>;

/// One finite admission-to-execution graph with DMA-local terminal error handling.
#[derive(Debug)]
pub struct Hardware<S = Pipeline> {
    graph: Graph<S>,
    count: usize,
}

impl<S> Hardware<S> {
    pub(super) fn new(commands: Commands, pipeline: S, count: usize) -> Self {
        Self {
            graph: Then::new(
                MapError::new(commands, Error::from as fn(_) -> _),
                Then::new(
                    Grant,
                    Selected(Then::new(
                        Fetch,
                        Then::new(CheckRanges, Then::new(Install, pipeline)),
                    )),
                ),
            ),
            count,
        }
    }
}

impl<M, SB, S> Stage<Dma<M, SB>> for Hardware<S>
where
    Graph<S>: Stage<Dma<M, SB>, Input = Vec<()>, Output = (), Error = Error>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = Option<Option<<Graph<S> as Stage<Dma<M, SB>>>::State>>;
    fn start((): ()) -> Self::State {
        Some(None)
    }
    fn step(&mut self, dma: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        let local = state.as_mut().ok_or(RunError::Exhausted)?;
        let local = local
            .get_or_insert_with(|| <Graph<S> as Stage<Dma<M, SB>>>::start(vec![(); self.count]));
        let result = self.graph.step(dma, local);
        if !matches!(result, Ok(StepOutcome::InProgress)) {
            *state = None;
        }
        match result {
            Ok(StepOutcome::Complete(Err(
                Error::Reset | Error::Halted | Error::ChannelFatal(_),
            ))) => Ok(StepOutcome::Complete(Ok(()))),
            result => result,
        }
    }
}
