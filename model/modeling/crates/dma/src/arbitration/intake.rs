use super::{Candidate, RoundRobin, port::Accept};
use crate::{ChannelTask, Dma, Error};
use hardware_interface_pipeline::{
    BranchError, Fan, FanState, Repeat, RunError, Stage, StepOutcome, StepResult, Then, ThenState,
};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};
use std::ops::ControlFlow;

type Accepted = ChannelTask<Command>;
type Round = Then<Fan<Candidate, RoundRobin, Accept>, FinishIntake>;
type RoundState = ThenState<
    Option<FanState<(), Option<ChannelId>, Option<ChannelId>, Option<Accepted>>>,
    Option<Option<Accepted>>,
>;

/// Accepts one command after non-consuming candidate arbitration.
#[derive(Debug)]
pub struct Intake {
    count: usize,
    rounds: Repeat<Round>,
}

impl Intake {
    pub(crate) fn new(count: usize) -> Self {
        Self::from_candidates(
            (0..count)
                .map(|index| Candidate::new(ChannelId::new(index)))
                .collect(),
        )
    }

    /// Uses precisely the supplied candidate channels and ordering.
    pub fn from_candidates(candidates: Vec<Candidate>) -> Self {
        let count = candidates.len();
        Self {
            count,
            rounds: Repeat::new(Then::new(
                Fan::new(candidates, RoundRobin::default(), vec![Accept]),
                FinishIntake(count),
            )),
        }
    }
}

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Intake {
    type Input = ();
    type Output = Accepted;
    type Error = Error;
    type State = Option<Option<RoundState>>;

    fn start((): ()) -> Self::State {
        None
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Accepted, Error> {
        let rounds = state.get_or_insert_with(|| {
            <Repeat<Round> as Stage<Dma<M, SB>>>::start(vec![(); self.count])
        });
        Ok(match self.rounds.step(device, rounds)? {
            StepOutcome::InProgress => StepOutcome::InProgress,
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map_err(Error::from)),
        })
    }
}

#[derive(Debug)]
struct FinishIntake(usize);

impl<M, SB> Stage<Dma<M, SB>> for FinishIntake {
    type Input = Vec<(usize, Option<Accepted>)>;
    type Output = ControlFlow<Accepted, Vec<()>>;
    type Error = BranchError<Error>;
    type State = Option<Option<Accepted>>;

    fn start(receipts: Self::Input) -> Self::State {
        Some(receipts.into_iter().next().and_then(|(_, value)| value))
    }

    fn step(
        &mut self,
        _: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        let accepted = state.take().ok_or(RunError::Exhausted)?;
        Ok(StepOutcome::Complete(Ok(match accepted {
            Some(task) => ControlFlow::Break(task),
            None => ControlFlow::Continue(vec![(); self.0]),
        })))
    }
}
