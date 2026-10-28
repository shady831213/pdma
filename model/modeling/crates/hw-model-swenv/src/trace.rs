use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug)]
struct TraceState<E> {
    events: Vec<TraceEntry<E>>,
    next_sequence: usize,
}

/// One owned event and its buffer-assigned insertion sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceEntry<E> {
    /// Saturating insertion sequence, restarted by clear.
    pub sequence: usize,
    /// Caller-constructed event payload.
    pub event: E,
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

    /// Appends an already constructed event and returns its insertion sequence.
    pub fn push(&self, event: E) -> usize {
        let mut state = self.lock();
        let sequence = state.next_sequence;
        state.events.push(TraceEntry { sequence, event });
        state.next_sequence = sequence.saturating_add(1);
        sequence
    }

    /// Returns an owned, coherent, insertion-ordered snapshot.
    ///
    /// Payload cloning runs under the buffer lock. `E::clone` must not reenter
    /// this buffer (including through a clone) or wait for an operation on it.
    pub fn snapshot(&self) -> Vec<TraceEntry<E>>
    where
        E: Clone,
    {
        self.lock().events.clone()
    }

    /// Removes events and resets the next sequence to zero.
    pub fn clear(&self) {
        let events = {
            let mut state = self.lock();
            state.next_sequence = 0;
            std::mem::take(&mut state.events)
        };
        drop(events);
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
    fn clear_releases_lock_before_dropping_payloads() {
        use std::sync::{
            Weak,
            atomic::{AtomicBool, Ordering},
        };
        struct Payload {
            state: Weak<Mutex<TraceState<Self>>>,
            unlocked: Arc<AtomicBool>,
        }
        impl Drop for Payload {
            fn drop(&mut self) {
                let state = self.state.upgrade().expect("trace still exists");
                self.unlocked
                    .store(state.try_lock().is_ok(), Ordering::SeqCst);
            }
        }
        // Given a payload whose destructor observes its own trace lock.
        let trace = TraceBuffer::new();
        let unlocked = Arc::new(AtomicBool::new(false));
        trace.push(Payload {
            state: Arc::downgrade(&trace.state),
            unlocked: Arc::clone(&unlocked),
        });
        // When clearing retained payloads.
        trace.clear();
        // Then destruction can safely access the trace again.
        assert!(unlocked.load(Ordering::SeqCst));
    }

    #[test]
    fn sequence_saturates_at_usize_max() {
        let trace = TraceBuffer {
            state: Arc::new(Mutex::new(TraceState {
                events: Vec::new(),
                next_sequence: usize::MAX,
            })),
        };
        trace.push(());
        trace.push(());
        assert_eq!(
            trace
                .snapshot()
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            [usize::MAX, usize::MAX]
        );
    }
}
