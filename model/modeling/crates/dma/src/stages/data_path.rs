#[cfg(test)]
mod tests;

use super::{Candidate, CheckRanges, Fetch, Retire, execution::Install, signal::Signals};
use crate::{
    ChunkInput, CommandRequest, CopyProgress, Dma, Error, ExecutionContext, FetchedCommand, Flow,
    Pipeline, RoundRobin,
};
use hardware_interface_pipeline::{
    Arbitrate, FanIn, MapError, Stage, StepOutcome, StepResult, Then,
};
use pdma_model_contract::dma::ChannelId;
use std::convert::Infallible;

type CopyGraph<S> = Then<CheckRanges, Then<Install, S, Flow<ChunkInput>>, Flow<CopyProgress>>;
type ExecutionGraph<S> = Then<Signals, CopyGraph<S>, Flow<FetchedCommand>>;
type TaskExecution<S> = Then<Fetch, Resources<ExecutionGraph<S>>, Flow<FetchedCommand>>;
type TaskPath<S> = Then<MapError<TaskExecution<S>, fn(Infallible) -> Error>, Retire, Flow<()>>;
type Commands<S> = FanIn<
    Candidate,
    Arbitrate<RoundRobin>,
    TaskPath<S>,
    (),
    Option<CommandRequest>,
    Option<CommandRequest>,
    (),
    Error,
>;
type Graph<S> = MapError<Commands<S>, fn(hardware_interface_pipeline::BranchError<Error>) -> Error>;

/// One finite admission-to-execution graph with DMA-local terminal error handling.
#[derive(Debug)]
pub struct Hardware<S = Pipeline> {
    graph: Graph<S>,
    count: usize,
    input: Option<Vec<()>>,
    busy: bool,
}

impl<S> Hardware<S> {
    pub(super) fn new(pipeline: S, count: usize) -> Self {
        Self {
            graph: MapError::new(
                FanIn::new(
                    (0..count)
                        .map(|index| Candidate::new(ChannelId::new(index)))
                        .collect(),
                    Arbitrate::snapshots(RoundRobin::default()),
                    Then::new(
                        MapError::new(
                            Then::new(
                                Fetch,
                                Resources(Then::new(
                                    Signals,
                                    Then::new(CheckRanges::default(), Then::new(Install, pipeline)),
                                )),
                            ),
                            impossible as fn(Infallible) -> Error,
                        ),
                        Retire,
                    ),
                ),
                Error::from as fn(_) -> _,
            ),
            count,
            input: None,
            busy: false,
        }
    }
}

impl<M, SB, S> Stage<Dma<M, SB>> for Hardware<S>
where
    Graph<S>: Stage<Dma<M, SB>, Input = Vec<()>, Output = Vec<(usize, ())>, Error = Error>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    fn step(&mut self, dma: &mut Dma<M, SB>, input: &mut Option<()>) -> StepResult<(), Error> {
        if !self.busy {
            if input.take().is_none() {
                return Ok(StepOutcome::InProgress);
            }
            self.input = Some(vec![(); self.count]);
            self.busy = true;
        }
        let result = self.graph.step(dma, &mut self.input);
        if matches!(result, Ok(StepOutcome::Complete(_))) {
            self.busy = false;
            self.input = None;
        }
        match result {
            Ok(StepOutcome::Complete(Err(
                Error::Reset | Error::Halted | Error::ChannelFatal(_),
            ))) => Ok(StepOutcome::Complete(Ok(()))),
            Ok(StepOutcome::Complete(result)) => Ok(StepOutcome::Complete(result.map(|_| ()))),
            Ok(StepOutcome::InProgress) => Ok(StepOutcome::InProgress),
            Err(error) => Err(error),
        }
    }
}

const fn impossible(error: Infallible) -> Error {
    match error {}
}

#[derive(Debug)]
struct Resources<S>(S);

impl<M, SB, S> Stage<Dma<M, SB>> for Resources<S>
where
    for<'a> S: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Flow<FetchedCommand>,
            Output = Flow<()>,
            Error = Infallible,
        >,
{
    type Input = Flow<FetchedCommand>;
    type Output = Flow<()>;
    type Error = Infallible;
    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        self.0.step(&mut dma.execution_context(), input)
    }
}
