use super::{
    Candidate, RoundRobin,
    engine::{Engine, EngineState},
    port::Frame,
};
use crate::{ChannelTask, Dma, Error, RunCommand};
use hardware_interface_pipeline::{
    BranchError, Fan, FanState, Repeat, RunError, Stage, StepOutcome, StepResult, Then, ThenState,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command, FaultStatusValues};
use std::ops::ControlFlow;

type Round<S> = Then<Fan<Candidate, RoundRobin, Engine<S>>, NextRound>;
type RoundState<T> = ThenState<Option<FanState<(), Option<ChannelId>, EngineState<T>, ()>>, ()>;

#[derive(Debug)]
enum Rounds<T> {
    Fresh,
    Running(Option<RoundState<T>>),
    Done,
}

/// Repeats candidate selection and one shared command engine.
#[derive(Debug)]
pub struct CommandArbiter<S> {
    count: usize,
    rounds: Repeat<Round<S>>,
}

/// Retains round progress and at most one exact continuation per channel.
#[derive(Debug)]
pub struct CommandArbiterState<T> {
    rounds: Rounds<T>,
    saved: Vec<Option<T>>,
}

impl<S> CommandArbiter<S> {
    pub(crate) fn new(count: usize, pipeline: S) -> Self {
        Self {
            count,
            rounds: Repeat::new(Then::new(
                Fan::new(
                    (0..count)
                        .map(|index| Candidate::new(ChannelId::new(index)))
                        .collect(),
                    RoundRobin::default(),
                    vec![Engine(RunCommand::new(pipeline))],
                ),
                NextRound,
            )),
        }
    }
}

impl<M: MemoryAccess<SB>, SB, S> Stage<Dma<M, SB>> for CommandArbiter<S>
where
    S: Stage<Dma<M, SB>, Input = ChannelTask<Command>, Output = (), Error = Error>,
{
    type Input = ();
    type Output = ();
    type Error = Error;
    type State = CommandArbiterState<S::State>;

    fn start((): ()) -> Self::State {
        CommandArbiterState {
            rounds: Rounds::Fresh,
            saved: Vec::new(),
        }
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        match state.rounds {
            Rounds::Done => return Err(RunError::Exhausted),
            Rounds::Fresh => {
                if state.saved.try_reserve_exact(self.count).is_err() {
                    state.rounds = Rounds::Done;
                    return Ok(StepOutcome::Complete(Err(Error::OperationAllocation)));
                }
                state.saved.resize_with(self.count, || None);
                let input = vec![(); self.count];
                let rounds = <Repeat<Round<S>> as Stage<Frame<'_, S::State, M, SB>>>::start(input);
                state.rounds = Rounds::Running(rounds);
            }
            Rounds::Running(_) => {}
        }
        for (index, (channel, saved)) in
            device.channels.iter_mut().zip(&mut state.saved).enumerate()
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
            return Ok(StepOutcome::InProgress);
        }
        let outcome = match &mut state.rounds {
            Rounds::Running(rounds) => self.rounds.step(
                &mut Frame {
                    device,
                    saved: &mut state.saved,
                },
                rounds,
            ),
            Rounds::Fresh | Rounds::Done => Err(RunError::Exhausted),
        };
        if !matches!(outcome, Ok(StepOutcome::InProgress)) {
            state.rounds = Rounds::Done;
            state.saved.clear();
            device.active = None;
            for channel in &mut device.channels {
                channel.parked = None;
            }
        }
        device.refresh_maintenance();
        Ok(match outcome? {
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map_err(Error::from)),
            StepOutcome::InProgress => StepOutcome::InProgress,
        })
    }
}

#[derive(Debug)]
struct NextRound;

impl<T, M, SB> Stage<Frame<'_, T, M, SB>> for NextRound {
    type Input = Vec<(usize, ())>;
    type Output = ControlFlow<(), Vec<()>>;
    type Error = BranchError<Error>;
    type State = ();

    fn start(_: Self::Input) {}

    fn step(
        &mut self,
        frame: &mut Frame<'_, T, M, SB>,
        (): &mut (),
    ) -> StepResult<Self::Output, Self::Error> {
        let count = frame.device.channels.len();
        let next = ControlFlow::Continue(vec![(); count]);
        Ok(StepOutcome::Complete(Ok(next)))
    }
}
