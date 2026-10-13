mod scope;

use crate::{Candidate, CommandRequest, Dma, Error, RoundRobin};
use hardware_interface_pipeline::{
    Arbitrate, FanIn, FanState, RunError, Stage, StepOutcome, StepResult,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, FaultStatusValues};
use scope::{CommandScope, ScopeState};

type Selection<S> = FanIn<Candidate, Arbitrate<RoundRobin>, CommandScope<S>>;
type SelectionState<T> = Option<FanState<(), Option<CommandRequest>, ScopeState<T>, ()>>;

/// Finite arbitration transaction and assembly-owned exact fault continuations.
#[derive(Debug)]
pub struct Commands<S, T> {
    count: usize,
    selection: Selection<S>,
    saved: Vec<Option<T>>,
}

/// Invocation-local candidate gathering and selected service progress.
#[derive(Debug)]
pub struct CommandState<T> {
    selection: SelectionState<T>,
    started: bool,
}

impl<S, T> Commands<S, T> {
    pub(crate) fn new(count: usize, pipeline: S) -> Self {
        Self {
            count,
            selection: FanIn::new(
                (0..count)
                    .map(|index| Candidate::new(ChannelId::new(index)))
                    .collect(),
                Arbitrate::snapshots(RoundRobin::default()),
                CommandScope(pipeline),
            ),
            saved: Vec::new(),
        }
    }
}

impl<M: MemoryAccess<SB>, SB, S, T> Stage<Dma<M, SB>> for Commands<S, T>
where
    S: Stage<Dma<M, SB>, Input = ChannelId, Output = (), Error = Error, State = T>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = CommandState<T>;

    fn start((): ()) -> Self::State {
        CommandState {
            selection: None,
            started: false,
        }
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        let fresh = !state.started;
        if !state.started {
            if self.saved.len() != self.count {
                if self.saved.try_reserve_exact(self.count).is_err() {
                    state.started = true;
                    return Ok(StepOutcome::Complete(Err(Error::OperationAllocation)));
                }
                self.saved.resize_with(self.count, || None);
            }
            state.selection =
                <Selection<S> as Stage<Frame<'_, T, M, SB>>>::start(vec![(); self.count]);
            state.started = true;
        } else if state.selection.is_none() {
            return Err(RunError::Exhausted);
        }
        if fresh {
            retire_stopped_progress(device);
        }
        for (index, (channel, saved)) in device.channels.iter_mut().zip(&mut self.saved).enumerate()
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
                *saved = None;
                channel.parked = None;
                channel.registers.values.fault_status = FaultStatusValues::new(0);
            }
        }
        if device.discard_aborted() {
            state.selection = None;
            return Ok(StepOutcome::Complete(Ok(())));
        }
        let outcome = self.selection.step(
            &mut Frame {
                device,
                saved: &mut self.saved,
            },
            &mut state.selection,
        );
        if matches!(outcome, Err(_) | Ok(StepOutcome::Complete(Err(_)))) {
            state.selection = None;
            self.saved.iter_mut().for_each(|saved| *saved = None);
            device.active = None;
            for channel in &mut device.channels {
                channel.parked = None;
                channel.progress = None;
            }
        } else if matches!(outcome, Ok(StepOutcome::Complete(Ok(_)))) {
            retire_stopped_progress(device);
        }
        device.refresh_maintenance();
        Ok(match outcome? {
            StepOutcome::Complete(result) => {
                StepOutcome::Complete(result.map(|_| ()).map_err(Error::from))
            }
            StepOutcome::InProgress => StepOutcome::InProgress,
        })
    }
}

fn retire_stopped_progress<M, SB>(device: &mut Dma<M, SB>) {
    let halted = device.halted();
    for (index, channel) in device.channels.iter_mut().enumerate() {
        if channel.progress.is_some() && (channel.resetting() || channel.halted() || halted) {
            channel.progress = None;
            if device
                .active
                .as_ref()
                .is_some_and(|task| task.channel().index() == index)
            {
                device.active = None;
            }
        }
    }
}

struct Frame<'a, T, M, SB> {
    device: &'a mut Dma<M, SB>,
    saved: &'a mut [Option<T>],
}

impl<T, M, SB> Stage<Frame<'_, T, M, SB>> for Candidate {
    type Input = ();
    type Output = Option<CommandRequest>;
    type Error = Error;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        frame: &mut Frame<'_, T, M, SB>,
        (): &mut (),
    ) -> StepResult<Self::Output, Error> {
        Ok(StepOutcome::Complete(self.request(frame.device)))
    }
}
