use crate::{
    ChannelContext, CheckRanges, Dma, Error, Fetch, NextChunk, ReadCopy, ResetControl, Resettable,
    ResponsePort, RoundRobin, RouteResponse, WriteCopy,
};
use hardware_interface_pipeline::{
    AfterEach, Arbitrate, BranchError, Dispatch, MapError, Operation, Repeat, Stage, StepOutcome,
    Then,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Response};
use std::{convert::Infallible, ops::ControlFlow};

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
        match context.channel.response.push(&context.memory, response) {
            Ok(true) => {
                context.channel.published(response, was_empty);
                StepOutcome::Complete(Ok(()))
            }
            Ok(false) => StepOutcome::Pending(()),
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

impl<M: MemoryAccess<SB>, S, SB> Stage<Dma<M, SB>> for RunCommand<S>
where
    S: Stage<Dma<M, SB>, Input = (), Output = (), Error = Error, Wait = ()>,
{
    type Input = ();
    type Output = ControlFlow<(), ()>;
    type Error = Error;
    type Wait = ();
    type State = Operation<S, Dma<M, SB>>;

    fn start((): ()) -> Self::State {
        Operation::new(())
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Error, ()> {
        if state.is_complete() {
            return StepOutcome::Exhausted;
        }
        if device.discard_aborted() {
            return StepOutcome::Progressed;
        }
        let outcome = state.step(&mut self.pipeline, device);
        if matches!(outcome, StepOutcome::Complete(_) | StepOutcome::Exhausted) {
            device.active = None;
        }
        device.refresh_maintenance();
        match outcome {
            StepOutcome::Complete(
                Ok(()) | Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted),
            ) => StepOutcome::Complete(Ok(ControlFlow::Continue(()))),
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
pub type Execution = Then<CheckRanges, Repeat<CopyLoop>>;

/// Application arbitration over the instance's command ports.
pub type Intake = MapError<Arbitrate<Fetch, RoundRobin>, fn(BranchError<Error>) -> Error>;

/// Single-destination response dispatch over the instance's publication ports.
pub type Responses =
    MapError<Dispatch<ResponsePort, RouteResponse>, fn(BranchError<Error>) -> Error>;

/// The same statically composed hardware is used by step and async executors.
pub type Pipeline = Then<Then<Intake, Resettable<AfterEach<Execution, UpdateIrq>>>, Responses>;

/// Continuously executes commands through one shared pipeline.
pub type CommandLoop = AfterEach<Repeat<RunCommand>, ResetControl>;

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
