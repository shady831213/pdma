use super::{HandlerIdentity, HandlerObservation, MainOutcome, notification_source};
use pdma_model_contract::semantic::{CommandId, CompletionStatus, EventErrorKind, IrqVector};
use pdma_model_host::DriverIrqRecord;

/// Verifies persistent handler identity, source routing, and drained records.
///
/// # Panics
/// Panics when the scenario outcome differs from the pinned completion/event order.
pub fn assert_observations(
    outcome: &MainOutcome,
    completion_vector: IrqVector,
    event_vector: IrqVector,
) {
    let [first, fault, final_report] = outcome.observations.as_slice() else {
        assert_eq!(outcome.observations.len(), 3);
        return;
    };
    assert_eq!(
        (first.identity, first.call, first.notification.vector()),
        (HandlerIdentity::Completion, 1, completion_vector)
    );
    assert_eq!(
        (fault.identity, fault.call, fault.notification.vector()),
        (HandlerIdentity::Event, 1, event_vector)
    );
    assert_eq!(
        (
            final_report.identity,
            final_report.call,
            final_report.notification.vector()
        ),
        (HandlerIdentity::Completion, 2, completion_vector)
    );
    assert_eq!(notification_source(first.notification), outcome.sources[0]);
    assert_eq!(notification_source(fault.notification), outcome.sources[1]);
    assert_eq!(
        notification_source(final_report.notification),
        outcome.sources[0]
    );
    assert_completion(first, outcome.commands[0]);
    assert_event(fault, outcome.commands[1]);
    assert_completion(final_report, outcome.commands[2]);
}

fn assert_completion(observation: &HandlerObservation, command: CommandId) {
    assert!(matches!(
        observation.records.as_slice(),
        [DriverIrqRecord::Completion(completion)]
            if completion.command_id() == command
                && completion.status() == CompletionStatus::Success
    ));
}

fn assert_event(observation: &HandlerObservation, command: CommandId) {
    assert!(matches!(
        observation.records.as_slice(),
        [DriverIrqRecord::Event(event)]
            if event.command_id() == command
                && event.error_kind() == EventErrorKind::OutOfBounds { len: 4, end: 8 }
    ));
}
