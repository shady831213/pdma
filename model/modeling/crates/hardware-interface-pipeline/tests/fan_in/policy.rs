use hardware_interface_pipeline::{Arbitration, ReadyStage, StepOutcome};

pub(super) struct Policy {
    cursor: usize,
    fixed: bool,
    wait: &'static str,
}

impl Policy {
    pub(super) const fn round_robin(wait: &'static str) -> Self {
        Self {
            cursor: 0,
            fixed: false,
            wait,
        }
    }
    pub(super) const fn fixed_priority(wait: &'static str) -> Self {
        Self {
            cursor: 0,
            fixed: true,
            wait,
        }
    }
}

impl<C, S> Arbitration<C, S> for Policy
where
    S: ReadyStage<C, Wait = &'static str>,
{
    type State = usize;
    fn start() -> usize {
        0
    }
    fn select_index(
        &mut self,
        context: &C,
        sources: &[S],
        visited: &mut usize,
    ) -> StepOutcome<usize, S::Error, &'static str> {
        if *visited == 0 && self.fixed {
            self.cursor = 0;
        }
        let index = self.cursor;
        let Some(source) = sources.get(index) else {
            return StepOutcome::Exhausted;
        };
        self.cursor = if index + 1 < sources.len() {
            index + 1
        } else {
            0
        };
        if source.ready(context) {
            return StepOutcome::Complete(Ok(index));
        }
        *visited += 1;
        if *visited < sources.len() {
            StepOutcome::Progressed
        } else {
            *visited = 0;
            StepOutcome::Pending(self.wait)
        }
    }
}
