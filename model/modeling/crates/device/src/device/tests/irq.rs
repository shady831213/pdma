use super::super::{DeviceCore, ModelStepStatus};
use crate::test_support::{
    TestQueue, configure, copy, doorbell, irq_write, poll_completion, submit,
};
use crate::{MappedDmaRegion, PdmaMemoryProvisioning};
use pdma_model_contract::abi::registers::IrqSourceRegisters;
use pdma_model_contract::semantic::ResourceConfig;

struct Fixture {
    core: DeviceCore,
    provisioning: PdmaMemoryProvisioning,
    queue: TestQueue,
    source: MappedDmaRegion,
}

fn core_with_queue() -> Fixture {
    let mut provisioning = PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut core = DeviceCore::new(provisioning.memory_access(), ResourceConfig::default());
    let source = provisioning.allocate(&mut core.memory, 4).expect("source");
    core.memory
        .write_object(source.object(), 0, &[1, 2, 3, 4])
        .expect("source bytes");
    let queue = configure(&mut core, &mut provisioning, 0, [4, 4, 4]);
    Fixture {
        core,
        provisioning,
        queue,
        source,
    }
}

impl Fixture {
    fn submit_copy(&mut self, command: u64) {
        let destination = self
            .provisioning
            .allocate(&mut self.core.memory, 4)
            .expect("destination");
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
