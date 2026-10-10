use std::{collections::BTreeSet, marker::PhantomData, ops::ControlFlow, vec::IntoIter};

use crate::{BranchError, Repeat, Stage, StepOutcome};

use super::Frame;

#[derive(Debug)]
pub(in crate::combinators) struct BranchPlan<M, I, O> {
    metadata: M,
    requests: IntoIter<(usize, I)>,
    outputs: Vec<O>,
}

impl<M, I, O> BranchPlan<M, I, O> {
    pub(in crate::combinators) fn new<E>(
        metadata: M,
        requests: Vec<(usize, I)>,
        ports: usize,
    ) -> Result<Self, BranchError<E>> {
        let mut selected = BTreeSet::new();
        for (index, _) in &requests {
            if *index >= ports {
                return Err(BranchError::InvalidIndex {
                    index: *index,
                    ports,
                });
            }
            if !selected.insert(*index) {
                return Err(BranchError::DuplicateIndex { index: *index });
            }
        }
        Ok(Self {
            metadata,
            requests: requests.into_iter(),
            outputs: Vec::new(),
        })
    }
}

#[derive(Debug)]
pub(in crate::combinators) struct RunBranchState<M, I, T, O> {
    plan: BranchPlan<M, I, O>,
    active: Option<(usize, T)>,
}

impl<M, I, T, O> RunBranchState<M, I, T, O> {
    pub(super) fn unstarted_index(&self) -> Option<usize> {
        if self.active.is_some() {
            return None;
        }
        self.plan
            .requests
            .as_slice()
            .first()
            .map(|(index, _)| *index)
    }
}

pub(in crate::combinators) type Branches<M> = Repeat<RunBranch<M>>;

pub(in crate::combinators) struct RunBranch<M>(PhantomData<fn() -> M>);

impl<M> RunBranch<M> {
    pub(super) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<C, S: Stage<C>, P, M> Stage<Frame<'_, C, S, P>> for RunBranch<M> {
    type Input = BranchPlan<M, S::Input, S::Output>;
    type Output = ControlFlow<(M, Vec<S::Output>), Self::Input>;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = RunBranchState<M, S::Input, S::State, S::Output>;

    fn start(input: Self::Input) -> Self::State {
        RunBranchState {
            plan: input,
            active: None,
        }
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, C, S, P>,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        let RunBranchState { mut plan, active } = state;
        let (index, active) = match active {
            Some(active) => active,
            None => match plan.requests.next() {
                Some((index, input)) => (index, S::start(input)),
                None => {
                    return StepOutcome::Complete(Ok(ControlFlow::Break((
                        plan.metadata,
                        plan.outputs,
                    ))));
                }
            },
        };
        let count = frame.ports.len();
        let Some(port) = frame.ports.get_mut(index) else {
            return StepOutcome::Complete(Err(BranchError::InvalidIndex {
                index,
                ports: count,
            }));
        };
        match port.step(frame.context, active) {
            StepOutcome::Progressed(active) => StepOutcome::Progressed(RunBranchState {
                plan,
                active: Some((index, active)),
            }),
            StepOutcome::Pending(active, wait) => StepOutcome::Pending(
                RunBranchState {
                    plan,
                    active: Some((index, active)),
                },
                wait,
            ),
            StepOutcome::Complete(Ok(output)) => {
                plan.outputs.push(output);
                StepOutcome::Complete(Ok(if plan.requests.len() == 0 {
                    ControlFlow::Break((plan.metadata, plan.outputs))
                } else {
                    ControlFlow::Continue(plan)
                }))
            }
            StepOutcome::Complete(Err(error)) => {
                StepOutcome::Complete(Err(BranchError::Application(error)))
            }
        }
    }
}
