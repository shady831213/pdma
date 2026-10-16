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
    pub reset: bool,
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
    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<()>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(()) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
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
}
pub(super) enum Behavior {
    Normal,
    Failure,
    Exhausted,
}
pub(super) struct Accept {
    pub behavior: Behavior,
    pending: Option<AcceptState>,
}
impl Accept {
    pub(super) const fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            pending: None,
        }
    }
}
impl Stage<Context> for Accept {
    type Input = usize;
    type Output = Option<Job>;
    type Error = &'static str;
    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<usize>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.pending.is_none() {
            self.pending = input.take().map(AcceptState::New);
        }
        let Some(active) = self.pending.take() else {
            return Ok(StepOutcome::InProgress);
        };
        if context.reset {
            return Ok(StepOutcome::Complete(Err("reset")));
        }
        match active {
            AcceptState::New(index) => {
                let Some(job) = context.queues.get_mut(index).and_then(VecDeque::pop_front) else {
                    return Ok(StepOutcome::Complete(Ok(None)));
                };
                context.accepted.push(index);
                self.pending = Some(AcceptState::Active(job));
                Ok(StepOutcome::InProgress)
            }
            AcceptState::Active(job) => {
                if context.blocked.contains(&job.channel) {
                    context.stalled = true;
                    self.pending = Some(AcceptState::Active(job));
                    return Ok(StepOutcome::InProgress);
                }
                match self.behavior {
                    Behavior::Normal => Ok(StepOutcome::Complete(Ok(Some(job)))),
                    Behavior::Failure => Ok(StepOutcome::Complete(Err("accept failed"))),
                    Behavior::Exhausted => {
                        self.pending = Some(AcceptState::Active(job));
                        self.behavior = Behavior::Normal;
                        Err(RunError::Exhausted)
                    }
                }
            }
        }
    }
}
pub(super) fn finish<S: Stage<Context>>(
    hardware: &mut S,
    state: &mut hardware_interface_pipeline::Operation<S, Context>,
    context: &mut Context,
) -> StepResult<S::Output, S::Error> {
    for _ in 0..100 {
        match state.step(hardware, context) {
            Ok(StepOutcome::InProgress) => {}
            terminal => return terminal,
        }
    }
    Err(RunError::BudgetExhausted)
}
