use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use std::collections::VecDeque;
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Job {
    pub channel: usize,
    pub value: u8,
}
#[derive(Default)]
pub(super) struct Context {
    pub queues: Vec<VecDeque<Job>>,
    pub observed: Vec<usize>,
    pub accepted: Vec<usize>,
    pub blocked: Vec<usize>,
    pub stalled: bool,
    pub updates: usize,
}
impl Context {
    pub(super) fn with_jobs(channels: &[usize]) -> Self {
        let count = channels.iter().max().map_or(0, |i| i + 1);
        let mut queues: Vec<VecDeque<Job>> = (0..count).map(|_| VecDeque::new()).collect();
        for &channel in channels {
            if let Some(queue) = queues.get_mut(channel) {
                queue.push_back(Job { channel, value: 7 });
            }
        }
        Self {
            queues,
            ..Self::default()
        }
    }
}
pub(super) struct Candidate(pub usize);
impl Stage<Context> for Candidate {
    type Input = ();
    type Output = Option<usize>;
    type Error = &'static str;
    type State = bool;
    fn start((): ()) -> bool {
        false
    }
    fn step(
        &mut self,
        context: &mut Context,
        done: &mut bool,
    ) -> StepResult<Self::Output, Self::Error> {
        if *done {
            return Err(RunError::Exhausted);
        }
        *done = true;
        context.observed.push(self.0);
        Ok(StepOutcome::Complete(Ok(context
            .queues
            .get(self.0)
            .filter(|q| !q.is_empty())
            .map(|_| self.0))))
    }
}
pub(super) enum AcceptState {
    New(usize),
    Active(Job),
    Done,
}
pub(super) enum Accept {
    Normal,
    Failure,
    Exhausted,
}
impl Stage<Context> for Accept {
    type Input = usize;
    type Output = Option<Job>;
    type Error = &'static str;
    type State = AcceptState;
    fn start(input: usize) -> AcceptState {
        AcceptState::New(input)
    }
    fn step(
        &mut self,
        context: &mut Context,
        state: &mut AcceptState,
    ) -> StepResult<Self::Output, Self::Error> {
        let active = std::mem::replace(state, AcceptState::Done);
        match active {
            AcceptState::New(index) => {
                let Some(job) = context.queues.get_mut(index).and_then(VecDeque::pop_front) else {
                    return Ok(StepOutcome::Complete(Ok(None)));
                };
                context.accepted.push(index);
                *state = AcceptState::Active(job);
                Ok(StepOutcome::InProgress)
            }
            AcceptState::Active(job) => {
                if context.blocked.contains(&job.channel) {
                    context.stalled = true;
                    *state = AcceptState::Active(job);
                    return Ok(StepOutcome::InProgress);
                }
                match self {
                    Self::Normal => Ok(StepOutcome::Complete(Ok(Some(job)))),
                    Self::Failure => Ok(StepOutcome::Complete(Err("accept failed"))),
                    Self::Exhausted => Err(RunError::Exhausted),
                }
            }
            AcceptState::Done => Err(RunError::Exhausted),
        }
    }
}
pub(super) fn finish<S: Stage<Context>>(
    hardware: &mut S,
    state: &mut S::State,
    context: &mut Context,
) -> StepResult<S::Output, S::Error> {
    for _ in 0..100 {
        match hardware.step(context, state) {
            Ok(StepOutcome::InProgress) => {}
            terminal => return terminal,
        }
    }
    Err(RunError::BudgetExhausted)
}
