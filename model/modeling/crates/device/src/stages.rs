use crate::{
    ChannelContext, CheckLengths, Device, Error, Fetch, NextChunk, ReadCopy, Resettable,
    ResponsePort, RoundRobin, RouteResponse, WriteCopy,
};
use hardware_interface_pipeline::{
    AfterEach, Arbitrate, BranchError, Dispatch, MapError, Repeat, Stage, StepOutcome, Then,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::{ChannelId, Response};
use std::convert::Infallible;

/// Publish a retained response; a full ring suspends only this stage.
#[derive(Debug, Default)]
pub struct Publish;

impl<M: MemoryAccess> Stage<ChannelContext<'_, M>> for Publish {
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
        context: &mut ChannelContext<'_, M>,
        response: &mut Response,
    ) -> StepOutcome<(), Error, ()> {
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let was_empty = context.channel.response.pointers().is_empty();
        match context.channel.response.push(context.memory, response) {
            Ok(true) => {
                context.channel.published(response, was_empty);
                StepOutcome::Complete(Ok(()))
            }
            Ok(false) => StepOutcome::Pending(()),
            Err(fault) => {
                context.channel.latch(fault);
                StepOutcome::Complete(Err(Error::Fault(fault)))
            }
        }
    }
}

/// Refresh observable IRQ causes from queue-owned response and fault state.
#[derive(Debug, Default)]
pub struct UpdateIrq;

impl<M> Stage<ChannelContext<'_, M>> for UpdateIrq {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();
    fn start((): ()) {}
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M>,
        (): &mut (),
    ) -> StepOutcome<Infallible, Infallible, Infallible> {
        context.channel.update_irq();
        StepOutcome::Progressed
    }
}

/// The same statically composed hardware is used by step and async executors.
pub type Pipeline = Then<Then<Intake, Resettable<AfterEach<Execution, UpdateIrq>>>, Responses>;

/// Application arbitration over the instance's command ports.
pub type Intake = MapError<Arbitrate<Fetch, RoundRobin>, fn(BranchError<Error>) -> Error>;

/// Single-destination response dispatch over the instance's publication ports.
pub type Responses =
    MapError<Dispatch<ResponsePort, RouteResponse>, fn(BranchError<Error>) -> Error>;

/// One composable prepare/read/write iteration.
pub type CopyLoop = Then<Then<NextChunk, ReadCopy>, WriteCopy>;

/// Length validation followed by bounded-memory copy iterations.
pub type Execution = Then<CheckLengths, Repeat<CopyLoop>>;

impl<M> Device<M> {
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

    /// Combines the instance's input ports into one shared copy and publication path.
    pub fn pipeline(&self) -> Pipeline {
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckLengths, Repeat::new(chunk));
        Then::new(
            Then::new(
                self.intake(),
                Resettable::new(AfterEach::new(execution, UpdateIrq)),
            ),
            self.responses(),
        )
    }
}
