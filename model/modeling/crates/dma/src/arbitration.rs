use crate::Error;
use hardware_interface_pipeline::{Arbitration, ReadyStage, StepOutcome};

/// PDMA policy: probe each channel once per idle round and rotate after a grant.
#[derive(Debug, Default)]
pub struct RoundRobin {
    next: usize,
}

impl<C, S: ReadyStage<C, Error = Error, Wait = ()>> Arbitration<C, S> for RoundRobin {
    type State = usize;

    fn start() -> usize {
        0
    }

    fn select_index(
        &mut self,
        context: &C,
        sources: &[S],
        mut visited: usize,
    ) -> StepOutcome<usize, (usize, usize), Error, ()> {
        if sources.is_empty() {
            return StepOutcome::Complete(Err(Error::Routing));
        }
        let index = self.next % sources.len();
        self.next = (index + 1) % sources.len();
        if sources
            .get(index)
            .is_some_and(|source| source.ready(context))
        {
            return StepOutcome::Complete(Ok((visited, index)));
        }
        visited += 1;
        if visited == sources.len() {
            StepOutcome::Pending(0, ())
        } else {
            StepOutcome::Progressed(visited)
        }
    }
}
