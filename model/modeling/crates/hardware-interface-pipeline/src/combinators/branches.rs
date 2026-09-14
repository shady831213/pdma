use std::{collections::BTreeSet, vec::IntoIter};

use crate::{BranchError, Stage, StepOutcome};

#[derive(Debug)]
pub(super) struct Branches<I, T, O> {
    requests: IntoIter<(usize, I)>,
    active: Option<(usize, T)>,
    outputs: Vec<O>,
    done: bool,
}

impl<I, T, O> Branches<I, T, O> {
    pub(super) fn new<E>(requests: Vec<(usize, I)>, ports: usize) -> Result<Self, BranchError<E>> {
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
            requests: requests.into_iter(),
            active: None,
            outputs: Vec::new(),
            done: false,
        })
    }

    pub(super) fn step<C, S>(
        &mut self,
        ports: &mut [S],
        context: &mut C,
    ) -> StepOutcome<Vec<O>, BranchError<S::Error>, S::Wait>
    where
        S: Stage<C, Input = I, State = T, Output = O>,
    {
        if self.done {
            return StepOutcome::Exhausted;
        }
        if self.active.is_none() {
            let Some((index, input)) = self.requests.next() else {
                self.done = true;
                return StepOutcome::Complete(Ok(std::mem::take(&mut self.outputs)));
            };
            self.active = Some((index, S::start(input)));
        }
        let Some((index, state)) = &mut self.active else {
            return StepOutcome::Exhausted;
        };
        let count = ports.len();
        let Some(port) = ports.get_mut(*index) else {
            self.done = true;
            return StepOutcome::Complete(Err(BranchError::InvalidIndex {
                index: *index,
                ports: count,
            }));
        };
        match port.step(context, state) {
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(wait) => StepOutcome::Pending(wait),
            StepOutcome::Complete(Ok(output)) => {
                self.active = None;
                self.outputs.push(output);
                if self.requests.len() == 0 {
                    self.done = true;
                    StepOutcome::Complete(Ok(std::mem::take(&mut self.outputs)))
                } else {
                    StepOutcome::Progressed
                }
            }
            StepOutcome::Complete(Err(error)) => {
                self.done = true;
                StepOutcome::Complete(Err(BranchError::Application(error)))
            }
            StepOutcome::Exhausted => {
                self.done = true;
                StepOutcome::Exhausted
            }
        }
    }
}
