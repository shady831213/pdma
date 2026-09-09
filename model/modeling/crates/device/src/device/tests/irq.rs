use super::super::{DeviceCore, ModelStepStatus};
use crate::test_support::{
    TestQueue, configure, copy, doorbell, irq_write, poll_completion, submit,
};
use hardware_interface_runtime::SharedMemoryAllocator;
use pdma_model_contract::abi::DmaRegion;
use pdma_model_contract::abi::registers::IrqSourceRegisters;
use pdma_model_contract::semantic::ResourceConfig;

struct Fixture {
    core: DeviceCore,
    allocator: SharedMemoryAllocator,
    queue: TestQueue,
    source: DmaRegion,
}

fn core_with_queue() -> Fixture {
    core_with_resources(ResourceConfig::default())
}

fn core_with_resources(resources: ResourceConfig) -> Fixture {
    let allocator = SharedMemoryAllocator::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        hardware_interface_runtime::MemoryAddress::new(0x1_0000),
    );
    let mut core = DeviceCore::new(allocator.memory_access(), resources);
    let source = crate::test_support::allocate(&allocator, 4);
    crate::memory::write(&core.memory, source.base(), &[1, 2, 3, 4]).expect("source bytes");
    let queue = configure(&mut core, &allocator, 0, [4, 4, 4]);
    Fixture {
        core,
        allocator,
        queue,
        source,
    }
}

impl Fixture {
    fn submit_copy(&mut self, command: u64) {
        let destination = crate::test_support::allocate(&self.allocator, 4);
        submit(
            &mut self.core,
            self.queue,
            copy(
                command,
                self.source.base().raw(),
                destination.base().raw(),
                4,
            ),
        );
        doorbell(&mut self.core, self.queue);
    }

    fn irq(&mut self, local: usize, value: u64) {
        irq_write(&mut self.core, self.queue, 0, local, value);
    }
}

#[test]
fn line_ack_preserves_a_cause_relatched_after_the_final_empty_poll() {
    // Given: a drained line-routed completion whose source is still enabled.
    let mut fixture = core_with_queue();
    fixture.irq(IrqSourceRegisters::ENABLED.offset(), 1);
    fixture.irq(IrqSourceRegisters::MASKED.offset(), 0);
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    assert!(fixture.core.take_irq_notification().is_some());
    assert!(poll_completion(&mut fixture.core, fixture.queue).is_some());
    assert_eq!(poll_completion(&mut fixture.core, fixture.queue), None);
    // When: another completion relatches the source before software acknowledges it.
    fixture.submit_copy(1);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    fixture.irq(IrqSourceRegisters::ACK.offset(), 1);
    // Then: the still-pending record keeps the line cause asserted.
    let snapshot = fixture.core.irq_snapshot().expect("relatched line cause");
    assert!(snapshot.asserted());
    assert_eq!(
        snapshot.cause().source(),
        pdma_model_contract::semantic::IrqSource::Completion(
            pdma_model_contract::semantic::CompletionRingId::new(0)
        )
    );
}

#[test]
fn line_ack_cannot_create_a_cause_for_a_disabled_source() {
    // Given: a disabled source with one published completion but no latched cause.
    let mut fixture = core_with_queue();
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    assert_eq!(fixture.core.irq_snapshot(), None);
    // When: software acknowledges before enabling and unmasking the source.
    fixture.irq(IrqSourceRegisters::ACK.offset(), 1);
    fixture.irq(IrqSourceRegisters::ENABLED.offset(), 1);
    fixture.irq(IrqSourceRegisters::MASKED.offset(), 0);
    // Then: queued records alone do not synthesize an interrupt cause.
    assert_eq!(fixture.core.irq_snapshot(), None);
}

#[test]
fn idle_step_refreshes_irq_output_after_mmio_unmask() {
    // Given: a completion latched while masked, with no remaining command work.
    let mut fixture = core_with_queue();
    fixture.irq(IrqSourceRegisters::ENABLED.offset(), 1);
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    fixture.irq(IrqSourceRegisters::MASKED.offset(), 0);
    assert!(!fixture.core.irq_notification_ready());

    // When: an idle hardware step evaluates the changed source.
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Idle));

    // Then: line output is asserted despite no command making progress.
    assert!(
        fixture
            .core
            .irq_snapshot()
            .expect("pending cause")
            .asserted()
    );
    assert!(fixture.core.irq_notification_ready());
}

#[test]
fn stalled_step_refreshes_irq_output_after_mmio_unmask() {
    // Given: a masked completion and a second command stalled behind its publication slot.
    let mut fixture =
        core_with_resources(ResourceConfig::default().with_completion_publication_slots(1));
    fixture.irq(IrqSourceRegisters::ENABLED.offset(), 1);
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    fixture.submit_copy(1);
    let stalled = Ok(ModelStepStatus::Stalled(
        crate::AsyncStallReason::CompletionPublicationPressure,
    ));
    assert_eq!(fixture.core.step(), stalled);
    fixture.irq(IrqSourceRegisters::MASKED.offset(), 0);
    assert!(!fixture.core.irq_notification_ready());

    // When: the retained command remains stalled during another hardware step.
    assert_eq!(fixture.core.step(), stalled);

    // Then: IRQ evaluation is not short-circuited by the command's pending outcome.
    assert!(fixture.core.irq_notification_ready());
    assert!(
        fixture
            .core
            .irq_snapshot()
            .expect("pending cause")
            .asserted()
    );
}

#[test]
fn repeated_idle_steps_do_not_consume_or_duplicate_a_message() {
    // Given: one message-routed completion.
    let mut fixture = core_with_queue();
    fixture.irq(IrqSourceRegisters::ROUTE_KIND.offset(), 1);
    fixture.irq(IrqSourceRegisters::ENABLED.offset(), 1);
    fixture.irq(IrqSourceRegisters::MASKED.offset(), 0);
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));

    // When: repeated idle evaluations happen before delivery accepts the message.
    for _ in 0..3 {
        assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Idle));
    }

    // Then: the original identity is delivered once and never regenerated.
    assert!(matches!(
        fixture.core.take_irq_notification(),
        Some(pdma_model_contract::semantic::IrqNotification::Message(message))
            if message.id().raw() == 0
    ));
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(fixture.core.take_irq_notification(), None);
}
