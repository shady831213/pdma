use crate::{
    ChannelContext, CheckRanges, CommandLoop, Dma, Error, Fetch, NextChunk, ReadCopy, Resettable,
    ResponsePort, RoundRobin, RouteResponse, WriteCopy,
};
use hardware_interface_pipeline::{
    AfterEach, Arbitrate, BranchError, Dispatch, MapError, Repeat, Stage, StepOutcome, Then,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Response};
use std::convert::Infallible;

/// Publish a retained response; a full ring suspends only this stage.
#[derive(Debug, Default)]
pub struct Publish;

impl<M: MemoryAccess<SB>, SB> Stage<ChannelContext<'_, M, SB>> for Publish {
    type Input = Response;
    type Output = ();
    type Error = Error;
    type Wait = ();
    type State = Response;
    fn start(input: Response) -> Response {
        input
    }
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        response: Response,
    ) -> StepOutcome<Response, (), Error, ()> {
        if context.channel.resetting() {
            return StepOutcome::Complete(Err(Error::Reset));
        }
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(response, ());
        }
        let was_empty = context.channel.response.pointers().is_empty();
        match context.channel.response.push(&context.memory, &response) {
            Ok(true) => {
                context.channel.published(&response, was_empty);
                StepOutcome::Complete(Ok(()))
            }
            Ok(false) => StepOutcome::Pending(response, ()),
            Err(fatal) => {
                context.channel.latch(fatal);
                StepOutcome::Complete(Err(Error::ChannelFatal(fatal)))
            }
        }
    }
}

/// Refresh observable IRQ causes from queue-owned response and fatal state.
#[derive(Debug, Default)]
pub struct UpdateIrq;

impl<M, SB> Stage<ChannelContext<'_, M, SB>> for UpdateIrq {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();
    fn start((): ()) {}
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        (): (),
    ) -> StepOutcome<(), Infallible, Infallible, Infallible> {
        context.channel.update_irq();
        StepOutcome::Progressed(())
    }
}

/// One composable prepare/read/write iteration.
pub type CopyLoop = Then<Then<NextChunk, ReadCopy>, WriteCopy>;

/// Length validation followed by bounded-memory copy iterations.
pub type Execution = Then<CheckRanges, Repeat<CopyLoop>>;

/// Application arbitration over the instance's command ports.
pub type Intake = MapError<Arbitrate<Fetch, RoundRobin>, fn(BranchError<Error>) -> Error>;

/// Single-destination response dispatch over the instance's publication ports.
pub type Responses =
    MapError<Dispatch<ResponsePort, RouteResponse>, fn(BranchError<Error>) -> Error>;

/// Shared execution and publication hardware for one granted command.
pub type Pipeline = Then<Resettable<AfterEach<Execution, UpdateIrq>>, Responses>;

impl<M, SB> Dma<M, SB> {
    /// Builds this device's lightweight command input ports for custom compositions.
    pub fn fetches(&self) -> Vec<Fetch> {
        (0..self.channels.len())
            .map(|index| Fetch::new(ChannelId::new(index)))
            .collect()
    }

    /// Builds command arbitration independently of the downstream transform.
    pub fn intake(&self) -> Intake {
        MapError::new(
            Arbitrate::new(self.fetches(), RoundRobin::default()),
            Error::from,
        )
    }

    /// Builds response dispatch with each destination bound to its owning channel.
    pub fn responses(&self) -> Responses {
        let ports = (0..self.channels.len())
            .map(|index| {
                Resettable::for_channel(ChannelId::new(index), AfterEach::new(Publish, UpdateIrq))
            })
            .collect();
        MapError::new(Dispatch::new(ports, RouteResponse), Error::from)
    }

    /// Builds the sustained command loop from this instance's hardware stages.
    pub fn command_loop(&self) -> CommandLoop {
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckRanges, Repeat::new(chunk));
        self.command_loop_with(Then::new(
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
            self.responses(),
        ))
    }

    /// Builds a complete command loop around one shared downstream composition.
    pub fn command_loop_with<S>(&self, pipeline: S) -> CommandLoop<S> {
        CommandLoop::new(self.channels.len(), pipeline)
    }
}
