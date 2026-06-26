//! Queue arbitration policy for runnable command queues.

use crate::ids::QueueId;

/// Named command-queue arbitration policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArbitrationPolicy {
    /// Equal-priority queues advance in runnable-list order.
    #[default]
    RoundRobin,
}

/// Deterministic selector for runnable queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueueArbiter {
    policy: ArbitrationPolicy,
}

impl QueueArbiter {
    /// Creates an arbiter for a named policy.
    pub const fn new(policy: ArbitrationPolicy) -> Self {
        Self { policy }
    }

    /// Returns the active arbitration policy.
    pub const fn policy(self) -> ArbitrationPolicy {
        self.policy
    }

    /// Selects the next runnable queue.
    pub fn select_next(self, runnable: &mut Vec<QueueId>) -> Option<QueueId> {
        match self.policy {
            ArbitrationPolicy::RoundRobin => {
                if runnable.is_empty() {
                    None
                } else {
                    Some(runnable.remove(0))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArbitrationPolicy, QueueArbiter};
    use crate::ids::QueueId;

    #[test]
    fn round_robin_selects_oldest_runnable_queue() {
        // Given: two runnable queues in arrival order.
        let arbiter = QueueArbiter::default();
        let mut runnable = vec![QueueId::new(0), QueueId::new(1)];

        // When: the arbiter selects the next queue.
        let selected = arbiter.select_next(&mut runnable);

        // Then: it selects the oldest runnable queue and leaves the peer runnable.
        assert_eq!(selected, Some(QueueId::new(0)));
        assert_eq!(runnable, vec![QueueId::new(1)]);
    }

    #[test]
    fn default_policy_is_round_robin() {
        // Given: a default arbiter.
        let arbiter = QueueArbiter::default();

        // When: software reads its policy.
        let policy = arbiter.policy();

        // Then: the v0 default is round robin.
        assert_eq!(policy, ArbitrationPolicy::RoundRobin);
    }
}
