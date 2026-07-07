//! IRQ contract vocabulary tests.

use pdma_model_contract::{
    CompletionRingId, IrqCause, IrqLine, IrqSnapshot, IrqSource, IrqVector, QueueId,
};

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
    let completion_cause = IrqCause::new(IrqLine::new(IrqVector::new(0)), completion);
    let event_cause = IrqCause::new(IrqLine::new(IrqVector::new(1)), event);

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
        IrqSource::Completion(CompletionRingId::new(1)),
    );

    // When: software observes an IRQ snapshot.
    let snapshot = IrqSnapshot::new(cause, true);

    // Then: pending cause and assertion state are explicit and separate.
    assert_eq!(snapshot.cause(), cause);
    assert!(snapshot.asserted());
}
