use super::super::{DeviceCore, ModelStepStatus};
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, ResourceConfig,
    desc::CopyRequest,
    queue::{EventRouting, QueueConfig, QueueConfigHandle, SoftwareOwnerId},
};

fn core_with_queue() -> (DeviceCore, QueueConfigHandle) {
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
    );
    let queue = core
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue");
    (core, queue)
}

fn submit_copy(core: &mut DeviceCore, queue: QueueConfigHandle, command: usize) {
    let source = if command == 0 {
        let source = core.dma.allocate(4).expect("source").object();
        core.dma
            .write_object(source, 0, &[1, 2, 3, 4])
            .expect("source write");
        source
    } else {
        pdma_model_contract::semantic::BufferId::new(0)
    };
    let destination = core.dma.allocate(4).expect("destination").object();
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

#[test]
fn line_ack_preserves_a_cause_relatched_after_the_final_empty_poll() {
    // Given: a drained line-routed completion whose source is still enabled.
    let (mut core, queue) = core_with_queue();
    let source = queue.completion_irq_source();
    core.enable_irq_source(source).expect("enable source");
    core.unmask_irq_source(source).expect("unmask source");
    submit_copy(&mut core, queue, 0);
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    assert!(core.take_irq_notification().is_some());
    assert!(core.poll_completion(queue).expect("completion").is_some());
    assert_eq!(core.poll_completion(queue), Ok(None));

    // When: another completion relatches the source before software acknowledges it.
    submit_copy(&mut core, queue, 1);
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    core.ack_irq_source(source).expect("ack source");

    // Then: the still-pending record keeps the line cause asserted.
    let snapshot = core.irq_snapshot().expect("relatched line cause");
    assert!(snapshot.asserted());
    assert_eq!(snapshot.cause().source(), source.source());
}

#[test]
fn line_ack_cannot_create_a_cause_for_a_disabled_source() {
    // Given: a disabled source with one published completion but no latched cause.
    let (mut core, queue) = core_with_queue();
    let source = queue.completion_irq_source();
    submit_copy(&mut core, queue, 0);
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    assert_eq!(core.irq_snapshot(), None);

    // When: software acknowledges before enabling and unmasking the source.
    core.ack_irq_source(source).expect("ack source");
    core.enable_irq_source(source).expect("enable source");
    core.unmask_irq_source(source).expect("unmask source");

    // Then: queued records alone do not synthesize an interrupt cause.
    assert_eq!(core.irq_snapshot(), None);
}
