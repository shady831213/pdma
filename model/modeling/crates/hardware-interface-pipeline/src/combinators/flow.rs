use crate::{Stage, ThenState};

use super::branches::RunBranchState;

pub(super) struct Frame<'a, C, S, P> {
    pub(super) context: &'a mut C,
    pub(super) ports: &'a mut [S],
    pub(super) policy: &'a mut P,
}

pub(super) type FlowState<I, M, D, T, O> =
    ThenState<Option<I>, ThenState<Option<RunBranchState<M, D, T, O>>, Option<(M, Vec<O>)>>>;

pub(super) fn start<C, S: Stage<C>>(_: &S, input: S::Input) -> S::State {
    S::start(input)
}
