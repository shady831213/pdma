use crate::{Stage, StepOutcome, Then, ThenState};

mod branches;
mod reduce;

pub(super) use branches::BranchPlan;
use branches::{Branches, RunBranch, RunBranchState};
use reduce::Reduce;

#[derive(Debug)]
pub(super) struct Fan<S, P> {
    pub(super) ports: Vec<S>,
    policy: P,
}

pub(super) struct Frame<'a, C, S, P> {
    pub(super) context: &'a mut C,
    pub(super) ports: &'a mut [S],
    pub(super) policy: &'a mut P,
}

pub(super) type FanState<I, M, D, T, O> =
    ThenState<Option<I>, ThenState<Option<RunBranchState<M, D, T, O>>, Option<(M, Vec<O>)>>>;

type Pipeline<A, M, F> = Then<A, Then<Branches<M>, Reduce<M, F>>>;

pub(super) const fn compose<A, M, F>(plan: A, reduce: F) -> Pipeline<A, M, F> {
    Then::new(
        plan,
        Then::new(Branches::new(RunBranch::new()), Reduce::new(reduce)),
    )
}

pub(super) fn start<C, S: Stage<C>>(_: &S, input: S::Input) -> S::State {
    S::start(input)
}

impl<S, P> Fan<S, P> {
    pub(super) const fn new(ports: Vec<S>, policy: P) -> Self {
        Self { ports, policy }
    }

    pub(super) fn unstarted_port<I, M, D, T, O>(
        &self,
        state: &FanState<I, M, D, T, O>,
    ) -> Option<&S> {
        match state {
            ThenState::Downstream(next) => match next {
                ThenState::Upstream(branches) => branches
                    .as_ref()
                    .and_then(RunBranchState::unstarted_index)
                    .and_then(|index| self.ports.get(index)),
                ThenState::Downstream(_) | ThenState::Done => None,
            },
            ThenState::Upstream(_) | ThenState::Done => None,
        }
    }

    pub(super) fn step<'a, C, A>(
        &'a mut self,
        context: &'a mut C,
        state: &mut A::State,
        mut pipeline: A,
    ) -> StepOutcome<A::Output, A::Error, A::Wait>
    where
        A: Stage<Frame<'a, C, S, P>>,
    {
        pipeline.step(
            &mut Frame {
                context,
                ports: &mut self.ports,
                policy: &mut self.policy,
            },
            state,
        )
    }
}
