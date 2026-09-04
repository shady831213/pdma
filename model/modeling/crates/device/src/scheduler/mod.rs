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

    pub(crate) fn runnable_len(&self) -> usize {
        self.runnable.len()
    }

    pub(crate) fn rotate_current(&mut self) {
        if let Some(queue) = self.runnable.pop_front() {
            self.runnable.push_back(queue);
        }
    }

    pub(crate) fn reset(&mut self) {
        self.runnable.clear();
    }
}

#[cfg(test)]
mod tests;
