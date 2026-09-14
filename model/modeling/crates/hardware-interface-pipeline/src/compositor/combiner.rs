use crate::{ReadyStage, Stage, StepOutcome};

/// Determines the first candidate for each fan-in operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Starts after the previous grant and wraps around the sources.
    RoundRobin,
    /// Starts at the first source for every operation.
    FixedPriority,
}

/// Selects one ready source and retains its grant until the operation terminates.
///
/// Each searching step probes one candidate. A full pass without a grant returns
/// the configured idle wait. Sources and policy stay fixed for the hardware's
/// lifetime; only the selected source has operation state.
#[derive(Debug)]
pub struct Combiner<S, W = ()> {
    sources: Vec<S>,
    selection: Selection,
    cursor: usize,
    idle_wait: W,
}

/// Retained state of one fan-in operation.
#[derive(Debug)]
pub enum CombinerState<S> {
    /// Number of candidates visited in the current unsuccessful pass.
    Searching(usize),
    /// A granted source with its retained operation state.
    Active {
        /// Index of the granted source.
        index: usize,
        /// Accepted operation owned by that source.
        state: S,
    },
    /// Terminal operation with no further interactions.
    Done,
}

impl<S, W> Combiner<S, W> {
    /// Owns the source population, selection policy, and notification used when idle.
    pub const fn new(sources: Vec<S>, selection: Selection, idle_wait: W) -> Self {
        Self {
            sources,
            selection,
            cursor: 0,
            idle_wait,
        }
    }
}

impl<C, S, W> Stage<C> for Combiner<S, W>
where
    S: ReadyStage<C, Wait = W>,
    W: Clone,
{
    type Input = ();
    type Output = S::Output;
    type Error = S::Error;
    type Wait = W;
    type State = CombinerState<S::State>;

    fn start((): ()) -> Self::State {
        CombinerState::Searching(0)
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, W> {
        match state {
            CombinerState::Searching(visited) => {
                if *visited == 0 {
                    match self.selection {
                        Selection::RoundRobin => {}
                        Selection::FixedPriority => self.cursor = 0,
                    }
                }
                let index = self.cursor;
                let Some(source) = self.sources.get(index) else {
                    *state = CombinerState::Done;
                    return StepOutcome::Exhausted;
                };
                let ready = source.ready(context);
                self.cursor = if index + 1 < self.sources.len() {
                    index + 1
                } else {
                    0
                };
                if !ready {
                    *visited = visited.saturating_add(1);
                    return if *visited < self.sources.len() {
                        StepOutcome::Progressed
                    } else {
                        *visited = 0;
                        StepOutcome::Pending(self.idle_wait.clone())
                    };
                }
                *state = CombinerState::Active {
                    index,
                    state: S::start(()),
                };
            }
            CombinerState::Active { .. } => {}
            CombinerState::Done => return StepOutcome::Exhausted,
        }
        let outcome = match state {
            CombinerState::Active { index, state } => self.sources.get_mut(*index).map_or_else(
                || StepOutcome::Exhausted,
                |source| source.step(context, state),
            ),
            CombinerState::Searching(_) | CombinerState::Done => StepOutcome::Exhausted,
        };
        match outcome {
            StepOutcome::Complete(_) | StepOutcome::Exhausted => *state = CombinerState::Done,
            StepOutcome::Progressed | StepOutcome::Pending(_) => {}
        }
        outcome
    }
}

impl<C, S, W> ReadyStage<C> for Combiner<S, W>
where
    S: ReadyStage<C, Wait = W>,
    W: Clone,
{
    fn ready(&self, context: &C) -> bool {
        self.sources.iter().any(|source| source.ready(context))
    }
}
