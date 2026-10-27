//! Trace insertion, snapshot ownership and concurrency contracts.

use hardware_interface_software::{TraceBuffer, TraceEntry};
use std::thread;

#[test]
fn constructing_an_event_can_read_and_append_to_the_same_buffer() {
    // Given a trace shared with event construction code.
    let trace = TraceBuffer::new();
    let build = || {
        assert!(trace.snapshot().is_empty());
        assert_eq!(trace.push("nested"), 0);
        "outer"
    };
    // When constructing the outer event before appending it.
    let sequence = trace.push(build());
    // Then nested insertion is complete before the outer insertion receives a sequence.
    assert_eq!(sequence, 1);
    assert_eq!(
        trace.snapshot(),
        [
            TraceEntry {
                sequence: 0,
                event: "nested"
            },
            TraceEntry {
                sequence: 1,
                event: "outer"
            },
        ]
    );
}

#[test]
fn concurrent_append_sequences_match_coherent_owned_snapshots() {
    // Given independently cloned handles to the same trace.
    let trace = TraceBuffer::new();
    // When four threads append distinct payloads and take snapshots concurrently.
    thread::scope(|scope| {
        for worker in 0..4 {
            let trace = trace.clone();
            scope.spawn(move || {
                for item in 0..64 {
                    let event = (worker, item);
                    let sequence = trace.push(event);
                    let snapshot = trace.snapshot();
                    assert_eq!(snapshot.get(sequence).map(|entry| entry.event), Some(event));
                    assert!(
                        snapshot
                            .iter()
                            .enumerate()
                            .all(|(index, entry)| index == entry.sequence)
                    );
                }
            });
        }
    });
    // Then every event is retained exactly once, and snapshots survive clear.
    let snapshot = trace.snapshot();
    assert_eq!(snapshot.len(), 256);
    let mut payloads: Vec<_> = snapshot.iter().map(|entry| entry.event).collect();
    payloads.sort_unstable();
    assert_eq!(
        payloads,
        (0..4)
            .flat_map(|worker| (0..64).map(move |item| (worker, item)))
            .collect::<Vec<_>>()
    );
    trace.clear();
    assert!(trace.snapshot().is_empty());
    assert_eq!(snapshot.len(), 256);
    assert_eq!(trace.push((9, 9)), 0);
}

#[test]
fn cleared_payload_destructor_can_append_after_sequence_reset() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload {
        trace: Option<TraceBuffer<Self>>,
        sequence: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            if let Some(trace) = self.trace.take() {
                let sequence = trace.push(Self {
                    trace: None,
                    sequence: Arc::clone(&self.sequence),
                });
                self.sequence.store(sequence, Ordering::SeqCst);
            }
        }
    }
    // Given a retained payload that appends on destruction.
    let trace = TraceBuffer::new();
    let sequence = Arc::new(AtomicUsize::new(usize::MAX));
    trace.push(Payload {
        trace: Some(trace.clone()),
        sequence: Arc::clone(&sequence),
    });
    // When clear removes that payload.
    trace.clear();
    // Then its destructor's append receives the reset sequence and stays retained.
    assert_eq!(sequence.load(Ordering::SeqCst), 0);
    assert_eq!(
        trace.push(Payload {
            trace: None,
            sequence
        }),
        1
    );
}
