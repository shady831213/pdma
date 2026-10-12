use super::{
    RoundRobin,
    port::{CommandOperation, ExecutionPort, Frame},
};
use crate::{ChannelTask, Dma, Error, RunCommand};
use hardware_interface_pipeline::{Arbitrate, FanInState, Repeat, Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command, FaultStatusValues};
use std::ops::ControlFlow;

/// One shared command engine with channel-grant arbitration and exact continuations.
#[derive(Debug)]
pub struct CommandArbiter<S> {
    engine: RunCommand<S>,
    turns: Repeat<Arbitrate<ExecutionPort, RoundRobin>>,
}

/// Arbitration progress and bounded retained command operations.
#[derive(Debug)]
pub struct CommandArbiterState<T> {
    turns: Option<FanInState<usize, bool, ControlFlow<(), ()>>>,
    operations: Vec<Option<T>>,
}

impl<S> CommandArbiter<S> {
    pub(crate) fn new(count: usize, pipeline: S) -> Self {
        Self {
            engine: RunCommand::new(pipeline),
            turns: Repeat::new(Arbitrate::new(
                (0..count)
                    .map(|index| ExecutionPort(ChannelId::new(index)))
                    .collect(),
                RoundRobin::default(),
            )),
        }
    }
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Dma<M, SB>> for CommandArbiter<S>
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error, Wait = ()>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type Wait = ();
    type State = CommandArbiterState<CommandOperation<S, M, SB>>;

    fn start((): ()) -> Self::State {
        CommandArbiterState {
            turns:
                <Repeat<Arbitrate<ExecutionPort, RoundRobin>> as Stage<Frame<'_, S, M, SB>>>::start(
                    (),
                ),
            operations: Vec::new(),
        }
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepOutcome<(), Error, ()> {
        if state.operations.is_empty() {
            if state
                .operations
                .try_reserve_exact(device.channels.len())
                .is_err()
            {
                return StepOutcome::Complete(Err(Error::OperationAllocation));
            }
            state.operations.resize_with(device.channels.len(), || None);
        }
        for (index, (channel, operation)) in device
            .channels
            .iter_mut()
            .zip(&mut state.operations)
            .enumerate()
        {
            let aborts = device.maintenance.as_ref().is_some_and(|maintenance| {
                maintenance.abort
                    && maintenance.result == Ok(false)
                    && maintenance
                        .frontiers
                        .iter()
                        .any(|frontier| frontier.channel.index() == index)
                    && channel
                        .parked
                        .as_ref()
                        .is_some_and(|task| task.value.contains(&Some(maintenance.context)))
            });
            if channel.parked.is_some() && (channel.resetting() || aborts) {
                *operation = None;
                channel.parked = None;
                channel.registers.values.fault_status = FaultStatusValues::new(0);
            }
        }
        if device.discard_aborted() {
            return StepOutcome::Progressed;
        }
        let outcome = self.turns.step(
            &mut Frame {
                device,
                engine: &mut self.engine,
                operations: &mut state.operations,
            },
            &mut state.turns,
        );
        device.refresh_maintenance();
        match outcome {
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map_err(Error::from)),
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(()) => StepOutcome::Pending(()),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}
