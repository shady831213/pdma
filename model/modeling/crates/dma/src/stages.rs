use crate::{
    ChannelContext, CheckLengths, Device, Error, Fetch, NextChunk, ReadCopy, ResetControl,
    Resettable, ResponsePort, RoundRobin, RouteResponse, WriteCopy,
};
use hardware_interface_pipeline::{
    AfterEach, Arbitrate, BranchError, Dispatch, MapError, Operation, Repeat, Stage, StepOutcome,
    Then,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::{ChannelId, Response};
use std::{convert::Infallible, ops::ControlFlow};

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
        if context.channel.resetting() {
            return StepOutcome::Complete(Err(Error::Reset));
        }
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

/// Ends one command after completion or channel-local termination.
#[derive(Debug)]
pub struct RunCommand<S = Pipeline> {
    pipeline: S,
}

impl<S> RunCommand<S> {
    /// Runs one composed command with shared execution ownership and terminal cleanup.
    pub const fn new(pipeline: S) -> Self {
        Self { pipeline }
    }
}

impl<M, S> Stage<Device<M>> for RunCommand<S>
where
    S: Stage<Device<M>, Input = (), Output = (), Error = Error, Wait = ()>,
{
    type Input = ();
    type Output = ControlFlow<(), ()>;
    type Error = Error;
    type Wait = ();
    type State = Operation<S, Device<M>>;

    fn start((): ()) -> Self::State {
        Operation::new(())
    }

    fn step(
        &mut self,
        device: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Error, ()> {
        if state.is_complete() {
            return StepOutcome::Exhausted;
        }
        let outcome = state.step(&mut self.pipeline, device);
        if matches!(outcome, StepOutcome::Complete(_) | StepOutcome::Exhausted) {
            device.active = None;
        }
        match outcome {
            StepOutcome::Complete(Ok(()) | Err(Error::Reset | Error::Fault(_) | Error::Halted)) => {
                StepOutcome::Complete(Ok(ControlFlow::Continue(())))
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(()) => StepOutcome::Pending(()),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}

/// One composable prepare/read/write iteration.
pub type CopyLoop = Then<Then<NextChunk, ReadCopy>, WriteCopy>;

/// Length validation followed by bounded-memory copy iterations.
pub type Execution = Then<CheckLengths, Repeat<CopyLoop>>;

/// Application arbitration over the instance's command ports.
pub type Intake = MapError<Arbitrate<Fetch, RoundRobin>, fn(BranchError<Error>) -> Error>;

/// Single-destination response dispatch over the instance's publication ports.
pub type Responses =
    MapError<Dispatch<ResponsePort, RouteResponse>, fn(BranchError<Error>) -> Error>;

/// The same statically composed hardware is used by step and async executors.
pub type Pipeline = Then<Then<Intake, Resettable<AfterEach<Execution, UpdateIrq>>>, Responses>;

/// Continuously executes commands through one shared pipeline.
pub type CommandLoop = AfterEach<Repeat<RunCommand>, ResetControl>;

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

    /// Builds the sustained command loop from this instance's hardware stages.
    pub fn command_loop(&self) -> CommandLoop {
        let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
        let execution = Then::new(CheckLengths, Repeat::new(chunk));
        AfterEach::new(
            Repeat::new(RunCommand::new(Then::new(
                Then::new(
                    self.intake(),
                    Resettable::new(AfterEach::new(execution, UpdateIrq)),
                ),
                self.responses(),
            ))),
            ResetControl,
        )
    }
}
