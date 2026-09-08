use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use pdma_model_contract::abi::mmio::{IrqCommand, IrqStateBit, MmioResult};
use pdma_model_contract::semantic::{
    DeviceEpoch, EventRouting, IrqLine, IrqRoute, IrqVector, QueueConfig, SoftwareOwnerId,
    TraceEventKind,
};

#[test]
fn owner_command_enables_a_registered_source_without_device_core() {
    // Given: an IRQ owner and a separately owned queue directory.
    let epoch = DeviceEpoch::initial();
    let mut irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    let mut queues = QueueBlock::default();
    queues
        .create_semantic(
            epoch,
            QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        )
        .expect("queue");
    let mut trace = TraceSink::default();
    irq.registers.expected_epoch = epoch.raw().into();
    irq.registers.command = IrqCommand::Enable.code().into();

    // When: the IRQ owner executes its own staged command.
    let changed = irq.execute_mmio(&queues, &mut trace);

    // Then: the source is enabled and its owned response reports the transition.
    assert!(changed);
    assert_eq!(irq.registers.result.raw(), MmioResult::Ok.code());
    assert_eq!(
        irq.registers.response.state.raw(),
        IrqStateBit::Present.mask() | IrqStateBit::Enabled.mask() | IrqStateBit::Masked.mask()
    );
}

#[test]
fn owner_claim_consumes_one_message_and_records_its_trace() {
    // Given: two queued message notifications on a registered source.
    let epoch = DeviceEpoch::new(7);
    let mut irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    let mut queues = QueueBlock::default();
    let queue = queues
        .create_semantic(
            epoch,
            QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        )
        .expect("queue");
    let source = queue.completion_irq_source().source();
    irq.configure_route(source, IrqRoute::Message(IrqVector::new(41)))
        .expect("inactive route");
    irq.enable(source);
    irq.unmask(source);
    irq.latch(source);
    irq.latch(source);
    let mut trace = TraceSink::default();
    trace.enable();
    irq.registers.command = IrqCommand::ClaimNotification.code().into();

    // When: the owner claims using its current epoch despite untouched selectors.
    let changed = irq.execute_mmio(&queues, &mut trace);

    // Then: exactly one message is consumed and emitted with the owner epoch and vector.
    assert!(changed);
    assert_eq!(irq.registers.result.raw(), MmioResult::Ok.code());
    assert_eq!(irq.registers.response.epoch.raw(), 7);
    assert_eq!(irq.registers.response.notification_kind.raw(), 2);
    assert_eq!(irq.registers.response.vector.raw(), 41);
    assert_eq!(irq.registers.response.message_id.raw(), 0);
    assert_eq!(irq.registers.response.pending_messages.raw(), 1);
    assert!(irq.notification_ready());
    assert_eq!(
        trace
            .events()
            .iter()
            .map(pdma_model_contract::semantic::TraceEvent::kind)
            .collect::<Vec<_>>(),
        [TraceEventKind::IrqMessageEmitted]
    );
}

#[test]
fn owner_route_command_uses_every_staged_selector() {
    // Given: an event source selected among thirteen separately registered queues.
    let epoch = DeviceEpoch::new(11);
    let mut irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    let mut queues = QueueBlock::default();
    for _ in 0..13 {
        queues
            .create_semantic(
                epoch,
                QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
            )
            .expect("queue");
    }
    irq.registers.expected_epoch = 11.into();
    irq.registers.source_id = 12.into();
    irq.registers.source_kind = 1.into();
    irq.registers.route_kind = 1.into();
    irq.registers.route_vector = 13.into();
    irq.registers.command = IrqCommand::ConfigureRoute.code().into();

    // When: the owner configures the selected source through its register command.
    let changed = irq.execute_mmio(&queues, &mut TraceSink::default());

    // Then: the response identifies the exact selected source and applied route.
    assert!(changed);
    assert_eq!(irq.registers.result.raw(), MmioResult::Ok.code());
    let response = irq.registers.response;
    assert_eq!(
        (
            response.epoch.raw(),
            response.source_id.raw(),
            response.source_kind.raw(),
            response.route_kind.raw(),
            response.vector.raw()
        ),
        (11, 12, 1, 1, 13)
    );
}
