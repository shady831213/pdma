use std::collections::VecDeque;

use pdma_model_contract::semantic::QueueId;

#[derive(Debug, Default)]
pub(crate) struct Scheduler {
    runnable: VecDeque<QueueId>,
}

impl Scheduler {
    pub(crate) fn enqueue(&mut self, queue: QueueId) {
        if !self.runnable.contains(&queue) {
            self.runnable.push_back(queue);
        }
    }

    pub(crate) fn next(&mut self) -> Option<QueueId> {
        self.runnable.pop_front()
    }

    pub(crate) fn current(&self) -> Option<QueueId> {
        self.runnable.front().copied()
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.runnable.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.runnable.len()
    }

    pub(crate) fn reset(&mut self) {
        self.runnable.clear();
    }
}
