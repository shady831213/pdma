use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug)]
struct TraceState<E> {
    events: Vec<E>,
    next_sequence: usize,
}

/// Clone-shared ordered event storage with saturating sequence allocation.
#[derive(Debug)]
pub struct TraceBuffer<E> {
    state: Arc<Mutex<TraceState<E>>>,
}

impl<E> TraceBuffer<E> {
    /// Creates an independent empty buffer whose first sequence is zero.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TraceState {
                events: Vec::new(),
                next_sequence: 0,
            })),
        }
    }

    /// Appends an event built from the next saturating sequence.
    pub fn push_with(&self, build: impl FnOnce(usize) -> E) {
        let mut state = self.lock();
        let sequence = state.next_sequence;
        state.events.push(build(sequence));
        state.next_sequence = sequence.saturating_add(1);
    }

    /// Returns a point-in-time ordered copy of all events.
    pub fn events(&self) -> Vec<E>
    where
        E: Clone,
    {
        self.lock().events.clone()
    }

    /// Removes events and resets the next sequence to zero.
    pub fn clear(&self) {
        let mut state = self.lock();
        state.events.clear();
        state.next_sequence = 0;
    }

    fn lock(&self) -> MutexGuard<'_, TraceState<E>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<E> Clone for TraceBuffer<E> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl<E> Default for TraceBuffer<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{TraceBuffer, TraceState};
    use std::sync::{Arc, Mutex};

    #[test]
    fn sequence_saturates_at_usize_max() {
        let trace = TraceBuffer {
            state: Arc::new(Mutex::new(TraceState {
                events: Vec::new(),
                next_sequence: usize::MAX,
            })),
        };
        trace.push_with(|sequence| sequence);
        trace.push_with(|sequence| sequence);
        assert_eq!(trace.events(), [usize::MAX, usize::MAX]);
    }
}
