use super::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use pdma_model_contract::abi::registers::{IrqSourceRegisters, QueueRegisters};
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, IrqNotification, IrqSource, QueueId,
};

fn sources() -> QueueBlock {
    let mut queues = QueueBlock::default();
    for index in 0..2 {
        queues
            .write_configuration(index, QueueRegisters::SQ_DEPTH.offset(), 2)
            .expect("queue");
    }
    queues
}

fn enable(queues: &mut QueueBlock, source: IrqSource, message: bool) {
    let source = queues.irq_source_mut(source).expect("queue source");
    for (register, value) in [
        (IrqSourceRegisters::ROUTE_KIND, u64::from(message)),
        (IrqSourceRegisters::ENABLED, 1),
        (IrqSourceRegisters::MASKED, 0),
    ] {
        source
            .write_register(
                IrqSourceRegisters::register(register.id()).expect("register"),
                value,
                false,
            )
            .expect("write");
    }
    source.latch();
}

fn completion(index: usize) -> IrqSource {
    IrqSource::Completion(CompletionRingId::new(index))
}

#[test]
fn update_selects_lines_before_messages_and_completion_sources_before_events() {
    // Given: pending messages, an event line and a higher-index completion line.
    let mut queues = sources();
    enable(&mut queues, completion(0), true);
    enable(&mut queues, IrqSource::Event(QueueId::new(0)), false);
    enable(&mut queues, completion(1), false);
    let mut controller = PdmaIrqController::new(DeviceEpoch::initial());

    // When: hardware evaluates all source outputs.
    controller.update(&queues, &mut TraceSink::default());

    // Then: line priority wins, using completion-before-event source order.
    let snapshot = controller.pending_snapshot(&queues).expect("line");
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), completion(1));
    assert_eq!(
        controller.take_notification(&mut queues),
        Some(IrqNotification::Line(snapshot)),
    );
    assert_eq!(
        controller.take_notification(&mut queues),
        Some(IrqNotification::Line(snapshot))
    );
}

#[test]
fn masked_pending_line_does_not_hide_an_asserted_source() {
    // Given: a masked lower completion and asserted higher completion.
    let mut queues = sources();
    enable(&mut queues, completion(0), false);
    queues
        .irq_source_mut(completion(0))
        .expect("source")
        .write_register(
            IrqSourceRegisters::register(IrqSourceRegisters::MASKED.id()).expect("mask register"),
            1,
            false,
        )
        .expect("mask");
    enable(&mut queues, completion(1), false);
    let mut controller = PdmaIrqController::new(DeviceEpoch::initial());

    // When: the hardware aggregates line output.
    controller.update(&queues, &mut TraceSink::default());

    // Then: the asserted source supplies the cause.
    let snapshot = controller.pending_snapshot(&queues).expect("line");
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), completion(1));
}

#[test]
fn ack_of_selected_line_does_not_consume_or_reselect_another_source() {
    // Given: two asserted line sources with the first selected.
    let mut queues = sources();
    enable(&mut queues, completion(0), false);
    enable(&mut queues, completion(1), false);
    let mut controller = PdmaIrqController::new(DeviceEpoch::initial());
    let mut trace = TraceSink::default();
    controller.update(&queues, &mut trace);

    // When: ACK clears the selected cause before another hardware update.
    queues
        .irq_source_mut(completion(0))
        .expect("source")
        .ack(false);

    // Then: no stale line escapes; only a subsequent update selects the other source.
    assert!(!controller.notification_ready(&queues));
    assert_eq!(controller.take_notification(&mut queues), None);
    controller.update(&queues, &mut trace);
    assert_eq!(
        controller
            .pending_snapshot(&queues)
            .expect("second line")
            .cause()
            .source(),
        completion(1)
    );
}

#[test]
fn repeated_update_preserves_pending_messages_until_taken() {
    // Given: two pending messages from one queue source.
    let mut queues = sources();
    enable(&mut queues, completion(0), true);
    queues
        .irq_source_mut(completion(0))
        .expect("source")
        .latch();
    let mut controller = PdmaIrqController::new(DeviceEpoch::initial());
    let mut trace = TraceSink::default();

    // When: hardware reevaluates repeatedly before each accepted notification.
    for expected in 0..2 {
        for _ in 0..3 {
            controller.update(&queues, &mut trace);
        }
        let Some(IrqNotification::Message(message)) = controller.take_notification(&mut queues)
        else {
            panic!("message output");
        };
        assert_eq!(message.id().raw(), expected);
        assert_eq!(message.source(), completion(0));
        assert_eq!(controller.take_notification(&mut queues), None);
    }

    // Then: no extra notification can be manufactured by further evaluation.
    controller.update(&queues, &mut trace);
    assert!(!controller.notification_ready(&queues));
    assert_eq!(controller.take_notification(&mut queues), None);
}

#[test]
fn remasking_selected_message_prevents_consumption_until_unmask_and_update() {
    // Given: a selected message output.
    let mut queues = sources();
    enable(&mut queues, completion(0), true);
    let mut controller = PdmaIrqController::new(DeviceEpoch::initial());
    let mut trace = TraceSink::default();
    controller.update(&queues, &mut trace);

    // When: software masks the source before accepting that output.
    queues
        .irq_source_mut(completion(0))
        .expect("source")
        .write_register(
            IrqSourceRegisters::register(IrqSourceRegisters::MASKED.id()).expect("mask register"),
            1,
            false,
        )
        .expect("mask");

    // Then: the pending message survives, retaining identity after unmask and evaluation.
    assert!(!controller.notification_ready(&queues));
    assert_eq!(controller.take_notification(&mut queues), None);
    queues
        .irq_source_mut(completion(0))
        .expect("source")
        .write_register(
            IrqSourceRegisters::register(IrqSourceRegisters::MASKED.id()).expect("mask register"),
            0,
            false,
        )
        .expect("unmask");
    controller.update(&queues, &mut trace);
    assert!(matches!(controller.take_notification(&mut queues),
        Some(IrqNotification::Message(message)) if message.id().raw() == 0));
}
