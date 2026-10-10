use super::{Frame, lifecycle};
use crate::{CommandTurn, Dma, Error, RunCommand};
use hardware_interface_pipeline::{Operation, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use std::ops::ControlFlow;

impl<M: MemoryAccess<SB>, SB, S> Stage<Frame<'_, Operation<S, Dma<M, SB>>, M, SB>> for RunCommand<S>
where
    S: Stage<Dma<M, SB>, Output = (), Error = Error, Wait = ()>,
{
    type Input = Operation<S, Dma<M, SB>>;
    type Output = ControlFlow<(), Self::Input>;
    type Error = Error;
    type Wait = ();
    type State = Self::Input;

    fn start(operation: Self::Input) -> Self::State {
        operation
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, Operation<S, Dma<M, SB>>, M, SB>,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Error, ()> {
        match self.step(frame.device, state) {
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map(|turn| match turn {
                CommandTurn::Finished => ControlFlow::Break(()),
                CommandTurn::Suspended(state) => ControlFlow::Continue(state),
            })),
            StepOutcome::Progressed(state) => StepOutcome::Progressed(state),
            StepOutcome::Pending(state, ()) => StepOutcome::Pending(state, ()),
        }
    }
}

#[derive(Debug)]
pub(super) struct Park;

impl<T, M, SB> Stage<Frame<'_, T, M, SB>> for Park {
    type Input = ControlFlow<(), T>;
    type Output = ControlFlow<(), ()>;
    type Error = Error;
    type Wait = ();
    type State = Self::Input;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, T, M, SB>,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Error, ()> {
        match state {
            ControlFlow::Break(()) => {}
            ControlFlow::Continue(continuation) => {
                let Some(task) = frame.device.active.as_ref() else {
                    return StepOutcome::Complete(Err(Error::Routing));
                };
                let index = task.channel.index();
                let Some(channel) = frame.device.channels.get(index) else {
                    return StepOutcome::Complete(Err(Error::Routing));
                };
                if channel.resetting() || lifecycle::aborts(frame.device, task) {
                    drop(continuation);
                    frame.device.active = None;
                    if let Some(channel) = frame.device.channels.get_mut(index) {
                        channel.registers.values.fault_status =
                            pdma_model_contract::dma::FaultStatusValues::new(0);
                    }
                } else {
                    let Some(slot) = frame.parked.get_mut(index) else {
                        return StepOutcome::Complete(Err(Error::Routing));
                    };
                    *slot = Some(continuation);
                    if let Some(channel) = frame.device.channels.get_mut(index) {
                        channel.parked = frame.device.active.take();
                    }
                }
            }
        }
        StepOutcome::Complete(Ok(ControlFlow::Continue(())))
    }
}
