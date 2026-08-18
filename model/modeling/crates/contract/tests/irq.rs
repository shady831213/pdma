//! IRQ contract vocabulary tests.

use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId,
    IrqNotification, IrqRoute, IrqSnapshot, IrqSource, IrqVector, QueueId,
};

const fn current(source: IrqSource) -> EpochIrqSource {
    EpochIrqSource::new(DeviceEpoch::initial(), source)
}

#[test]
fn irq_line_identifies_system_vector() {
    // Given: a typed IRQ vector assigned by the host system model.
    let vector = IrqVector::new(3);

    // When: the vector is wrapped as a line identity.
    let line = IrqLine::new(vector);

    // Then: the line preserves the vector without exposing raw integer mixing.
    assert_eq!(line.vector(), vector);
    assert_eq!(line.vector().raw(), 3);
}

#[test]
fn irq_source_distinguishes_completion_and_event_paths() {
    // Given: one completion source and one queue-local event source.
    let completion = IrqSource::Completion(CompletionRingId::new(2));
    let event = IrqSource::Event(QueueId::new(7));

    // When: both sources are carried as IRQ causes.
    let completion_cause = IrqCause::new(IrqLine::new(IrqVector::new(0)), current(completion));
    let event_cause = IrqCause::new(IrqLine::new(IrqVector::new(1)), current(event));

    // Then: source identity remains typed and comparable.
    assert_eq!(completion_cause.source(), completion);
    assert_eq!(event_cause.source(), event);
    assert_ne!(completion_cause.line(), event_cause.line());
}

#[test]
fn irq_snapshot_records_line_pending_and_asserted_state() {
    // Given: a pending completion IRQ on an asserted line.
    let cause = IrqCause::new(
        IrqLine::new(IrqVector::new(4)),
        current(IrqSource::Completion(CompletionRingId::new(1))),
    );

    // When: software observes an IRQ snapshot.
    let snapshot = IrqSnapshot::new(cause, true);

    // Then: pending cause and assertion state are explicit and separate.
    assert_eq!(snapshot.cause(), cause);
    assert!(snapshot.asserted());
}

#[test]
fn irq_route_distinguishes_line_and_message_delivery() {
    // Given: one host-observable line and one message-signaled vector.
    let line = IrqLine::new(IrqVector::new(2));
    let vector = IrqVector::new(9);

    // When: each delivery mechanism is represented as an IRQ route.
    let line_route = IrqRoute::Line(line);
    let message_route = IrqRoute::Message(vector);

    // Then: the routes retain distinct delivery semantics and typed identities.
    assert_eq!(line_route, IrqRoute::Line(line));
    assert_eq!(message_route, IrqRoute::Message(vector));
    assert_ne!(line_route, message_route);
}

#[test]
fn irq_message_preserves_each_discrete_delivery_identity() {
    // Given: two message-signaled notifications for the same PDMA source and vector.
    let source = IrqSource::Event(QueueId::new(4));
    let vector = IrqVector::new(7);
    let first = IrqMessage::new(IrqMessageId::new(0), vector, current(source));
    let second = IrqMessage::new(IrqMessageId::new(1), vector, current(source));

    // When: both are carried across the common IRQ notification boundary.
    let first_notification = IrqNotification::Message(first);
    let second_notification = IrqNotification::Message(second);

    // Then: the messages remain distinct without encoding PCIe address/data details.
    assert_eq!(first.id().raw(), 0);
    assert_eq!(first.vector(), vector);
    assert_eq!(first.source(), source);
    assert_ne!(first_notification, second_notification);
}

#[test]
fn irq_notification_resolves_line_and_message_vectors() {
    // Given: line and message notifications routed to different host vectors.
    let line_vector = IrqVector::new(5);
    let message_vector = IrqVector::new(11);
    let source = IrqSource::Event(QueueId::new(2));
    let line = IrqNotification::Line(IrqSnapshot::new(
        IrqCause::new(IrqLine::new(line_vector), current(source)),
        true,
    ));
    let message = IrqNotification::Message(IrqMessage::new(
        IrqMessageId::new(0),
        message_vector,
        current(source),
    ));

    // When/Then: the common boundary resolves the configured vector for either route.
    assert_eq!(line.vector(), line_vector);
    assert_eq!(message.vector(), message_vector);
}
