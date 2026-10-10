use std::{cell::RefCell, collections::BTreeMap, collections::VecDeque};

use hardware_interface_pipeline::{ReadyStage, Stage, StepOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Job {
    Value(u8),
    Fail,
    End,
}

#[derive(Default)]
pub(super) struct Context {
    pub(super) queues: BTreeMap<u8, VecDeque<Job>>,
    pub(super) probes: RefCell<Vec<u8>>,
    pub(super) consumed: Vec<u8>,
    pub(super) transformed: Vec<u8>,
    pub(super) updated: Vec<u8>,
    pub(super) released: bool,
}

impl Context {
    pub(super) fn enqueue(&mut self, id: u8, jobs: impl IntoIterator<Item = Job>) {
        self.queues.entry(id).or_default().extend(jobs);
    }
}

pub(super) struct Queue {
    pub(super) id: u8,
    pub(super) delayed: bool,
}

impl Queue {
    pub(super) const fn new(id: u8) -> Self {
        Self { id, delayed: false }
    }
}

pub(super) enum State {
    Fresh,
    Held(u8),
}

impl Stage<Context> for Queue {
    type Input = ();
    type Output = u8;
    type Error = &'static str;
    type Wait = &'static str;
    type State = State;

    fn start((): ()) -> State {
        State::Fresh
    }

    fn step(
        &mut self,
        context: &mut Context,
        state: State,
    ) -> StepOutcome<Self::State, u8, &'static str, &'static str> {
        match state {
            State::Fresh => {
                let job = context.queues.entry(self.id).or_default().pop_front();
                context.consumed.push(self.id);
                match job {
                    Some(Job::Value(value)) if self.delayed => {
                        StepOutcome::Progressed(State::Held(value))
                    }
                    Some(Job::Value(value)) => StepOutcome::Complete(Ok(value)),
                    Some(Job::Fail) => StepOutcome::Complete(Err("source fault")),
                    Some(Job::End) | None => StepOutcome::Complete(Err("source ended")),
                }
            }
            State::Held(value) => {
                if context.released {
                    StepOutcome::Complete(Ok(value))
                } else {
                    StepOutcome::Pending(state, "source release")
                }
            }
        }
    }
}

impl ReadyStage<Context> for Queue {
    fn ready(&self, context: &Context) -> bool {
        context.probes.borrow_mut().push(self.id);
        context
            .queues
            .get(&self.id)
            .is_some_and(|queue| !queue.is_empty())
    }
}
