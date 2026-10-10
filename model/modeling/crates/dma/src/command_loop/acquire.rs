use super::Frame;
use crate::{ChannelTask, Dma, Error, Fetch};
use hardware_interface_pipeline::{Operation, ReadyStage, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};
use std::{marker::PhantomData, ops::ControlFlow};

#[derive(Debug)]
pub(super) struct SelectionPort(pub ChannelId);

impl<T, M, SB> Stage<Frame<'_, T, M, SB>> for SelectionPort {
    type Input = ();
    type Output = ChannelId;
    type Error = Error;
    type Wait = ();
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        _: &mut Frame<'_, T, M, SB>,
        (): (),
    ) -> StepOutcome<(), ChannelId, Error, ()> {
        StepOutcome::Complete(Ok(self.0))
    }
}

impl<T, M, SB> ReadyStage<Frame<'_, T, M, SB>> for SelectionPort {
    fn ready(&self, frame: &Frame<'_, T, M, SB>) -> bool {
        Fetch::new(self.0).eligible(frame.device)
    }
}

#[derive(Debug)]
pub(super) struct Acquire<S>(PhantomData<fn() -> S>);

impl<S> Acquire<S> {
    pub(super) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Frame<'_, Operation<S, Dma<M, SB>>, M, SB>> for Acquire<S>
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error, Wait = ()>,
{
    type Input = ChannelId;
    type Output = ControlFlow<(), Operation<S, Dma<M, SB>>>;
    type Error = Error;
    type Wait = ();
    type State = ChannelId;

    fn start(channel: ChannelId) -> ChannelId {
        channel
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, Operation<S, Dma<M, SB>>, M, SB>,
        channel: ChannelId,
    ) -> StepOutcome<ChannelId, Self::Output, Error, ()> {
        if !Fetch::new(channel).eligible(frame.device) {
            return StepOutcome::Complete(Ok(ControlFlow::Break(())));
        }
        let Some(slot) = frame.parked.get_mut(channel.index()) else {
            return StepOutcome::Complete(Err(Error::Routing));
        };
        if let Some(operation) = slot.take() {
            frame.device.active = frame
                .device
                .channels
                .get_mut(channel.index())
                .and_then(|channel| channel.parked.take());
            return StepOutcome::Complete(Ok(ControlFlow::Continue(operation)));
        }
        match Fetch::new(channel).step(frame.device, ()) {
            StepOutcome::Complete(Ok(task)) => {
                StepOutcome::Complete(Ok(ControlFlow::Continue(Operation::new(task))))
            }
            StepOutcome::Complete(Err(Error::Reset | Error::ChannelFatal(_) | Error::Halted))
            | StepOutcome::Pending((), ()) => StepOutcome::Complete(Ok(ControlFlow::Break(()))),
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::Progressed(()) => StepOutcome::Progressed(channel),
        }
    }
}
