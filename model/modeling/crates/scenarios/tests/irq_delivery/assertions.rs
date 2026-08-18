use super::support::{HandlerIdentity, HandlerObservation, RecordKind};
use pdma_model_contract::semantic::{IrqSource, IrqVector, TraceEvent, TraceEventKind};
use tokio::sync::mpsc;

pub async fn receive_observations(
    receiver: &mut mpsc::Receiver<HandlerObservation>,
    count: usize,
) -> Vec<HandlerObservation> {
    let mut observations = Vec::with_capacity(count);
    for _ in 0..count {
        let Some(observation) = receiver.recv().await else {
            break;
        };
        observations.push(observation);
    }
    observations
}

/// Verifies handler identity, source routing, records, and depth for distinct vectors.
///
/// # Panics
/// Panics if any observation differs from the expected five-turn sequence.
pub fn assert_distinct_observations(
    observations: &[HandlerObservation],
    vector_a: IrqVector,
    vector_b: IrqVector,
) {
    let identities = observations
        .iter()
        .map(|observation| observation.identity)
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        [
            HandlerIdentity::A,
            HandlerIdentity::B,
            HandlerIdentity::A,
            HandlerIdentity::B,
            HandlerIdentity::A,
        ]
    );
    for observation in observations {
        assert_eq!(observation.entry_depth, 1);
        assert_eq!(observation.exit_depth, 0);
        match observation.identity {
            HandlerIdentity::A => {
                assert_eq!(observation.vector, vector_a);
                assert!(matches!(observation.source, IrqSource::Completion(_)));
                assert_eq!(observation.records, [RecordKind::Completion]);
            }
            HandlerIdentity::B => {
                assert_eq!(observation.vector, vector_b);
                assert!(matches!(observation.source, IrqSource::Event(_)));
                assert_eq!(observation.records, [RecordKind::Event]);
            }
            HandlerIdentity::Shared => {}
        }
    }
}

/// Verifies that one persistent handler services completion and event sources.
///
/// # Panics
/// Panics unless exactly two correctly ordered source observations are present.
pub fn assert_shared_source_observations(observations: &[HandlerObservation], vector: IrqVector) {
    let [completion, event] = observations else {
        assert_eq!(observations.len(), 2);
        return;
    };
    assert_eq!(completion.identity, HandlerIdentity::Shared);
    assert_eq!(completion.call, 1);
    assert_eq!(completion.vector, vector);
    assert!(matches!(completion.source, IrqSource::Completion(_)));
    assert_eq!(completion.records, [RecordKind::Completion]);
    assert_eq!(event.identity, HandlerIdentity::Shared);
    assert_eq!(event.call, 2);
    assert_eq!(event.vector, vector);
    assert!(matches!(event.source, IrqSource::Event(_)));
    assert_eq!(event.records, [RecordKind::Event]);
    assert!(
        observations
            .iter()
            .all(|observation| observation.entry_depth == 1 && observation.exit_depth == 0)
    );
}

/// Verifies two discrete messages and the second redundant source drain.
///
/// # Panics
/// Panics unless both handler turns preserve identity, depth, and drain semantics.
pub fn assert_redundant_observations(observations: &[HandlerObservation], vector: IrqVector) {
    let [first, second] = observations else {
        assert_eq!(observations.len(), 2);
        return;
    };
    assert_eq!(first.call, 1);
    assert_eq!(first.records, [RecordKind::Event, RecordKind::Event]);
    assert_eq!(second.call, 2);
    assert!(second.records.is_empty());
    assert!(observations.iter().all(|observation| {
        observation.identity == HandlerIdentity::Shared
            && observation.vector == vector
            && observation.entry_depth == 1
            && observation.exit_depth == 0
    }));
}

/// Verifies the single Line completion handler observation.
///
/// # Panics
/// Panics unless one completion entered and exited the expected vector handler.
pub fn assert_line_observation(observations: &[HandlerObservation], vector: IrqVector) {
    let [observation] = observations else {
        assert_eq!(observations.len(), 1);
        return;
    };
    assert_eq!(observation.identity, HandlerIdentity::Shared);
    assert_eq!(observation.call, 1);
    assert_eq!(observation.vector, vector);
    assert!(matches!(observation.source, IrqSource::Completion(_)));
    assert_eq!(observation.records, [RecordKind::Completion]);
    assert_eq!(observation.entry_depth, 1);
    assert_eq!(observation.exit_depth, 0);
}

pub fn trace_count(events: &[TraceEvent], kind: TraceEventKind) -> usize {
    events.iter().filter(|event| event.kind() == kind).count()
}

/// Verifies that each expected trace kind appears after its predecessor.
///
/// # Panics
/// Panics if an expected trace kind is absent or appears before the preceding kind.
pub fn assert_trace_order(events: &[TraceEvent], expected: &[TraceEventKind]) {
    let mut previous = None;
    for kind in expected {
        let position = events
            .iter()
            .position(|event| event.kind() == *kind)
            .unwrap_or(events.len());
        assert!(
            position < events.len(),
            "expected lifecycle event is traced"
        );
        assert!(previous.is_none_or(|previous| previous < position));
        previous = Some(position);
    }
}

/// Verifies the complete device and host Line delivery lifecycle.
///
/// # Panics
/// Panics if lifecycle order or the single Line assertion/deassertion/ack counts differ.
pub fn assert_line_lifecycle(device: &[TraceEvent], host: &[TraceEvent]) {
    assert_trace_order(
        device,
        &[
            TraceEventKind::CompletionPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqLineAsserted,
            TraceEventKind::CompletionPolled,
            TraceEventKind::IrqDeviceAck,
            TraceEventKind::IrqLineDeasserted,
        ],
    );
    assert_eq!(trace_count(device, TraceEventKind::IrqLineAsserted), 1);
    assert_eq!(trace_count(device, TraceEventKind::IrqLineDeasserted), 1);
    assert_eq!(trace_count(device, TraceEventKind::IrqDeviceAck), 1);
    assert_trace_order(
        host,
        &[
            TraceEventKind::IrqHandlerStarted,
            TraceEventKind::IrqHandlerCompleted,
        ],
    );
}

/// Verifies two discrete Message deliveries without a Line assertion.
///
/// # Panics
/// Panics if event/message/poll/ack order, exact counts, or route isolation differ.
pub fn assert_redundant_message_lifecycle(device: &[TraceEvent]) {
    assert_trace_order(
        device,
        &[
            TraceEventKind::EventPublished,
            TraceEventKind::IrqSourceLatched,
            TraceEventKind::IrqMessageEmitted,
            TraceEventKind::EventPolled,
            TraceEventKind::IrqDeviceAck,
        ],
    );
    assert!(
        !device
            .iter()
            .any(|event| event.kind() == TraceEventKind::IrqLineAsserted)
    );
    assert_eq!(trace_count(device, TraceEventKind::EventPublished), 2);
    assert_eq!(trace_count(device, TraceEventKind::IrqMessageEmitted), 2);
    assert_eq!(trace_count(device, TraceEventKind::IrqDeviceAck), 2);
}
