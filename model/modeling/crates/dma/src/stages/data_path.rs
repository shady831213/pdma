#[cfg(test)]
mod tests;

use super::opcodes::{Branches, branches};
use super::{Candidate, Retire};
use crate::{CommandRequest, Completion, Dma, Error, Flow, Pipeline, RoundRobin};
use hw_model_pipeline::{
    Arbitrate, BranchError, FanIn, MapError, Stage, StepOutcome, StepResult, Then,
};
use pdma_model_contract::dma::ChannelId;
use std::convert::Infallible;

type TaskPath<S> = Then<
    MapError<Branches<S>, fn(BranchError<Infallible>) -> Error>,
    Retire,
    Vec<(usize, Flow<Completion>)>,
>;
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
type Graph<S> = MapError<Commands<S>, fn(hw_model_pipeline::BranchError<Error>) -> Error>;

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
                            branches(pipeline),
                            (|error| match error {
                                BranchError::Application(error) => match error {},
                                BranchError::InvalidIndex { .. }
                                | BranchError::DuplicateIndex { .. }
                                | BranchError::Unavailable { .. }
                                | BranchError::EmptyInput
                                | BranchError::InputCount { .. }
                                | BranchError::OutputCount { .. } => Error::Routing,
                            }) as fn(BranchError<Infallible>) -> Error,
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
