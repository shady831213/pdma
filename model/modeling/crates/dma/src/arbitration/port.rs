use crate::{ChannelTask, Dma, Error, Fetch, RunCommand};
use hardware_interface_pipeline::{Operation, ReadyStage, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};
use std::ops::ControlFlow;

pub(super) type CommandOperation<S, M, SB> = Operation<RunCommand<S>, Dma<M, SB>>;

pub(super) struct Frame<'a, S, M: MemoryAccess<SB>, SB>
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error, Wait = ()>,
{
    pub device: &'a mut Dma<M, SB>,
    pub engine: &'a mut RunCommand<S>,
    pub operations: &'a mut [Option<CommandOperation<S, M, SB>>],
}

#[derive(Debug)]
pub(super) struct ExecutionPort(pub ChannelId);

impl<M: MemoryAccess<SB>, SB, S> ReadyStage<Frame<'_, S, M, SB>> for ExecutionPort
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error, Wait = ()>,
{
    fn ready(&self, frame: &Frame<'_, S, M, SB>) -> bool {
        Fetch::new(self.0).eligible(frame.device)
    }
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Frame<'_, S, M, SB>> for ExecutionPort
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error, Wait = ()>,
{
    type Input = ();
    type Output = ControlFlow<(), ()>;
    type Error = Error;
    type Wait = ();
    type State = bool;

    fn start((): ()) -> bool {
        false
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, S, M, SB>,
        started: &mut bool,
    ) -> StepOutcome<Self::Output, Error, ()> {
        let Some(slot) = frame.operations.get_mut(self.0.index()) else {
            return StepOutcome::Complete(Err(Error::Routing));
        };
        if !*started {
            if slot.is_some() {
                frame.device.active = frame
                    .device
                    .channels
                    .get_mut(self.0.index())
                    .and_then(|channel| channel.parked.take());
            } else {
                match Fetch::new(self.0).step(frame.device, &mut ()) {
                    StepOutcome::Complete(Ok(task)) => *slot = Some(Operation::new(task)),
                    StepOutcome::Complete(Err(
                        Error::Reset | Error::ChannelFatal(_) | Error::Halted,
                    )) => return StepOutcome::Complete(Ok(ControlFlow::Continue(()))),
                    StepOutcome::Complete(Err(error)) => return StepOutcome::Complete(Err(error)),
                    StepOutcome::Pending(()) => return StepOutcome::Pending(()),
                    StepOutcome::Progressed => return StepOutcome::Progressed,
                    StepOutcome::Exhausted => return StepOutcome::Exhausted,
                }
            }
            *started = true;
            return StepOutcome::Progressed;
        }
        let Some(operation) = slot.as_mut() else {
            return StepOutcome::Complete(Err(Error::Routing));
        };
        let outcome = operation.step(frame.engine, frame.device);
        if operation.is_complete() {
            *slot = None;
        } else if let Some(channel) = frame.device.channels.get_mut(self.0.index())
            && channel.registers.values.fault_status.faulted()
        {
            channel.parked = frame.device.active.take();
            return StepOutcome::Complete(Ok(ControlFlow::Continue(())));
        }
        outcome
    }
}
