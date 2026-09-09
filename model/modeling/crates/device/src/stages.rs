use crate::{Device, Error, ReadCopy, WriteCopy};
use hardware_interface_pipeline::{AfterEach, Stage, StepOutcome, Then};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::{Command, Response};
use std::convert::Infallible;

/// Take one published descriptor from command queue state.
#[derive(Debug, Default)]
pub struct Fetch;

impl<M: MemoryAccess> Stage<Device<M>> for Fetch {
    type Input = ();
    type Output = Command;
    type Error = Error;
    type Wait = ();
    type State = ();
    fn start((): ()) {}
    fn step(&mut self, context: &mut Device<M>, (): &mut ()) -> StepOutcome<Command, Error, ()> {
        if context.channel.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        match context.channel.command.pop(&context.memory) {
            Ok(Some(command)) => StepOutcome::Complete(Ok(command)),
            Ok(None) => StepOutcome::Pending(()),
            Err(fault) => {
                context.channel.latch(fault);
                StepOutcome::Complete(Err(Error::Fault(fault)))
            }
        }
    }
}

/// Publish a retained response; a full ring suspends only this stage.
#[derive(Debug, Default)]
pub struct Publish;

impl<M: MemoryAccess> Stage<Device<M>> for Publish {
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
        context: &mut Device<M>,
        response: &mut Response,
    ) -> StepOutcome<(), Error, ()> {
        if context.channel.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        match context.channel.response.push(&context.memory, response) {
            Ok(true) => StepOutcome::Complete(Ok(())),
            Ok(false) => StepOutcome::Pending(()),
            Err(fault) => {
                context.channel.latch(fault);
                StepOutcome::Complete(Err(Error::Fault(fault)))
            }
        }
    }
}

/// Refresh the level line from queue-owned response and fault state.
#[derive(Debug, Default)]
pub struct UpdateIrq;

impl<M> Stage<Device<M>> for UpdateIrq {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();
    fn start((): ()) {}
    fn step(
        &mut self,
        context: &mut Device<M>,
        (): &mut (),
    ) -> StepOutcome<Infallible, Infallible, Infallible> {
        context.channel.update_irq();
        StepOutcome::Progressed
    }
}

/// The same statically composed hardware is used by step and async executors.
pub type Pipeline = AfterEach<Then<Then<Fetch, Then<ReadCopy, WriteCopy>>, Publish>, UpdateIrq>;

/// Compose fetch, nested copy execution, publication and interrupt aggregation.
pub const fn pipeline() -> Pipeline {
    let execution = Then::new(ReadCopy, WriteCopy);
    AfterEach::new(Then::new(Then::new(Fetch, execution), Publish), UpdateIrq)
}
