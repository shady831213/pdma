use super::super::{DeviceCore, ModelStepStatus};
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, ResourceConfig,
    desc::CopyRequest,
    queue::{EventRouting, QueueConfig, QueueConfigHandle, SoftwareOwnerId},
};

struct Fixture {
    core: DeviceCore,
    provisioning: crate::runtime::PdmaMemoryProvisioning,
    queue: QueueConfigHandle,
}

fn core_with_queue() -> Fixture {
    let provisioning = crate::runtime::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut core = DeviceCore::new(provisioning.memory_access(), ResourceConfig::default());
    let queue = core
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue");
    Fixture {
        core,
        provisioning,
        queue,
    }
}

impl Fixture {
    fn submit_copy(&mut self, command: usize) {
        let core = &mut self.core;
        let queue = self.queue;
        let source = if command == 0 {
            let source = self
                .provisioning
                .allocate(&mut core.memory, 4)
                .expect("source")
                .object();
            core.memory
                .write_object(source, 0, &[1, 2, 3, 4])
                .expect("source write");
            source
        } else {
            pdma_model_contract::semantic::BufferId::new(0)
        };
        let destination = self
            .provisioning
            .allocate(&mut core.memory, 4)
            .expect("destination")
            .object();
        core.submit_command(
            queue,
            Command::new(
                CommandId::new(command),
                CommandOperation::Copy(CopyRequest::new(source, destination, 4)),
            ),
        )
        .expect("copy submission");
        core.ring_doorbell(queue).expect("doorbell");
    }
}

#[test]
fn line_ack_preserves_a_cause_relatched_after_the_final_empty_poll() {
    // Given: a drained line-routed completion whose source is still enabled.
    let mut fixture = core_with_queue();
    let queue = fixture.queue;
    let source = queue.completion_irq_source();
    fixture
        .core
        .enable_irq_source(source)
        .expect("enable source");
    fixture
        .core
        .unmask_irq_source(source)
        .expect("unmask source");
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    assert!(fixture.core.take_irq_notification().is_some());
    assert!(
        fixture
            .core
            .poll_completion(queue)
            .expect("completion")
            .is_some()
    );
    assert_eq!(fixture.core.poll_completion(queue), Ok(None));

    // When: another completion relatches the source before software acknowledges it.
    fixture.submit_copy(1);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    fixture.core.ack_irq_source(source).expect("ack source");

    // Then: the still-pending record keeps the line cause asserted.
    let snapshot = fixture.core.irq_snapshot().expect("relatched line cause");
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), source.source());
}

#[test]
fn line_ack_cannot_create_a_cause_for_a_disabled_source() {
    // Given: a disabled source with one published completion but no latched cause.
    let mut fixture = core_with_queue();
    let source = fixture.queue.completion_irq_source();
    fixture.submit_copy(0);
    assert_eq!(fixture.core.step(), Ok(ModelStepStatus::Progressed));
    assert_eq!(fixture.core.irq_snapshot(), None);

    // When: software acknowledges before enabling and unmasking the source.
    fixture.core.ack_irq_source(source).expect("ack source");
    fixture
        .core
        .enable_irq_source(source)
        .expect("enable source");
    fixture
        .core
        .unmask_irq_source(source)
        .expect("unmask source");

    // Then: queued records alone do not synthesize an interrupt cause.
    assert_eq!(fixture.core.irq_snapshot(), None);
}
