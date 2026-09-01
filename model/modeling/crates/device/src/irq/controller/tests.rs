use super::PdmaIrqController;
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqLine, IrqMessage, IrqMessageId,
    IrqNotification, IrqRoute, IrqSource, IrqVector,
};

const EPOCH: DeviceEpoch = DeviceEpoch::initial();

const fn current(source: IrqSource) -> EpochIrqSource {
    EpochIrqSource::new(EPOCH, source)
}

#[test]
fn latch_asserts_enabled_unmasked_source() {
    // Given: an enabled and unmasked completion IRQ source.
    let line = IrqLine::new(IrqVector::new(0));
    let source = IrqSource::Completion(CompletionRingId::new(1));
    let mut controller = PdmaIrqController::new(EPOCH, line);
    controller.enable(source);
    controller.unmask(source);

    // When: the device latches the source.
    controller.latch(source);

    // Then: software can observe an asserted pending IRQ for the source.
    let snapshot = controller
        .pending_snapshot()
        .expect("enabled source should latch pending IRQ");
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), source);
    assert_eq!(snapshot.cause().line(), line);
}

#[test]
fn masked_source_latches_without_asserting_line() {
    // Given: an enabled but masked completion IRQ source.
    let line = IrqLine::new(IrqVector::new(0));
    let source = IrqSource::Completion(CompletionRingId::new(2));
    let mut controller = PdmaIrqController::new(EPOCH, line);
    controller.enable(source);

    // When: the device latches the source.
    controller.latch(source);

    // Then: pending cause is preserved while line assertion is suppressed.
    let snapshot = controller
        .pending_snapshot()
        .expect("masked source should remain pending");
    assert!(!snapshot.asserted());
    assert_eq!(snapshot.cause().source(), source);
}

#[test]
fn ack_clears_pending_without_changing_mask_state() {
    // Given: a masked pending IRQ source.
    let line = IrqLine::new(IrqVector::new(0));
    let source = IrqSource::Completion(CompletionRingId::new(3));
    let mut controller = PdmaIrqController::new(EPOCH, line);
    controller.enable(source);
    controller.latch(source);

    // When: software acknowledges the source and the device latches it again.
    controller.ack(source, false);
    controller.latch(source);

    // Then: the source remains masked after ack and still does not assert.
    let snapshot = controller
        .pending_snapshot()
        .expect("source should latch again after ack");
    assert!(!snapshot.asserted());
}

#[test]
fn disabled_source_does_not_latch() {
    // Given: a disabled IRQ source.
    let source = IrqSource::Completion(CompletionRingId::new(4));
    let mut controller = PdmaIrqController::new(EPOCH, IrqLine::new(IrqVector::new(0)));

    // When: the device attempts to latch the disabled source.
    controller.latch(source);

    // Then: no pending IRQ is recorded.
    assert_eq!(controller.pending_snapshot(), None);
}

#[test]
fn disabled_pending_source_stays_pending_without_asserting_line() {
    // Given: an enabled pending source whose line is asserted.
    let source = IrqSource::Completion(CompletionRingId::new(5));
    let mut controller = PdmaIrqController::new(EPOCH, IrqLine::new(IrqVector::new(0)));
    controller.enable(source);
    controller.unmask(source);
    controller.latch(source);

    // When: software disables that source before acking it.
    controller.disable(source);

    // Then: pending cause remains visible but delivery is deasserted.
    let snapshot = controller
        .pending_snapshot()
        .expect("disabled pending source should stay visible");
    assert!(!snapshot.asserted());
}

#[test]
fn remasking_pending_source_deasserts_line() {
    // Given: an asserted pending source.
    let source = IrqSource::Completion(CompletionRingId::new(6));
    let mut controller = PdmaIrqController::new(EPOCH, IrqLine::new(IrqVector::new(0)));
    controller.enable(source);
    controller.unmask(source);
    controller.latch(source);

    // When: software masks that source.
    controller.mask(source);

    // Then: pending cause remains visible but delivery is suppressed.
    let snapshot = controller
        .pending_snapshot()
        .expect("masked pending source should stay visible");
    assert!(!snapshot.asserted());
}

#[test]
fn asserted_source_takes_priority_over_masked_pending_source() {
    // Given: a masked lower source and an unmasked higher source are both pending.
    let masked = IrqSource::Completion(CompletionRingId::new(0));
    let asserted = IrqSource::Completion(CompletionRingId::new(1));
    let mut controller = PdmaIrqController::new(EPOCH, IrqLine::new(IrqVector::new(0)));
    controller.enable(masked);
    controller.latch(masked);
    controller.enable(asserted);
    controller.unmask(asserted);
    controller.latch(asserted);

    // When: software observes the pending IRQ snapshot.
    let snapshot = controller
        .pending_snapshot()
        .expect("pending sources should produce a snapshot");

    // Then: the asserted source is selected instead of the masked lower source.
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), asserted);
}

#[test]
fn message_route_preserves_each_trigger_without_coalescing() {
    // Given: an enabled message-signaled completion source.
    let source = IrqSource::Completion(CompletionRingId::new(7));
    let vector = IrqVector::new(5);
    let mut controller = PdmaIrqController::new(EPOCH, IrqLine::new(IrqVector::new(0)));
    controller
        .configure_route(source, IrqRoute::Message(vector))
        .expect("inactive source route is configurable");
    controller.enable(source);
    controller.unmask(source);

    // When: the source triggers twice before software handles either message.
    controller.latch(source);
    controller.latch(source);

    // Then: two distinct messages are delivered in order and no third remains.
    let first = controller
        .take_notification()
        .expect("first message remains pending");
    let second = controller
        .take_notification()
        .expect("second message remains pending");
    assert_eq!(
        first,
        IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(0),
            vector,
            current(source),
        ))
    );
    assert_eq!(
        second,
        IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(1),
            vector,
            current(source),
        ))
    );
    assert_eq!(controller.take_notification(), None);
}

#[test]
fn masked_message_route_releases_all_triggers_after_unmask() {
    // Given: an enabled but masked message-signaled completion source.
    let source = IrqSource::Completion(CompletionRingId::new(8));
    let mut controller = PdmaIrqController::new(EPOCH, IrqLine::new(IrqVector::new(0)));
    controller
        .configure_route(source, IrqRoute::Message(IrqVector::new(6)))
        .expect("inactive source route is configurable");
    controller.enable(source);

    // When: two triggers arrive while masked and software later unmasks the source.
    controller.latch(source);
    controller.latch(source);
    assert_eq!(controller.take_notification(), None);
    controller.unmask(source);

    // Then: both discrete messages remain available for delivery.
    assert!(matches!(
        controller.take_notification(),
        Some(IrqNotification::Message(_))
    ));
    assert!(matches!(
        controller.take_notification(),
        Some(IrqNotification::Message(_))
    ));
    assert_eq!(controller.take_notification(), None);
}
