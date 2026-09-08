use super::super::{DeviceCore, ModelStepStatus};
use crate::AsyncStallReason;
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, DeviceEpoch, ResourceConfig, WholeDeviceReset,
    desc::CopyRequest,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};

const fn queue_config(completion_depth: usize) -> QueueConfig {
    QueueConfig::new(
        2,
        completion_depth,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(0),
    )
}

fn submit_copy(
    core: &mut DeviceCore,
    command: usize,
) -> pdma_model_contract::semantic::QueueConfigHandle {
    let queue = core.create_queue(queue_config(1)).expect("queue");
    let source = core.memory.allocate(1).expect("source").object();
    let destination = core.memory.allocate(1).expect("destination").object();
    core.memory
        .write_object(source, 0, &[0x5a])
        .expect("source write");
    core.submit_command(
        queue,
        Command::new(
            CommandId::new(command),
            CommandOperation::Copy(CopyRequest::new(source, destination, 1)),
        ),
    )
    .expect("copy submission");
    core.ring_doorbell(queue).expect("doorbell");
    queue
}

#[test]
fn reset_preserves_persistent_state_and_clears_resettable_hardware() {
    // Given: one completed copy, trace history, DMA bytes, and an epoch-zero queue.
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
    );
    core.enable_tracing();
    let queue = submit_copy(&mut core, 0);
    let destination = pdma_model_contract::semantic::BufferId::new(1);
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    let generation = core.memory.generation();
    let trace = core.trace_events();

    // When: the sole reset runner advances the device epoch.
    let epoch = core
        .reset_runner(WholeDeviceReset::new(DeviceEpoch::initial()))
        .expect("reset");

    // Then: persistent DMA/trace survive while work, IRQ, and queue identity restart.
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(core.memory.generation(), generation);
    assert_eq!(core.memory.read_object(destination, 0, 1), Ok(vec![0x5a]));
    assert_eq!(core.trace_events(), trace);
    assert!(core.ring_doorbell(queue).is_err());
    let fresh = core.create_queue(queue_config(1)).expect("fresh queue");
    assert_eq!(fresh.epoch(), epoch);
    assert_eq!(fresh.queue_id().raw(), 0);
    assert_eq!(core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(core.irq_snapshot(), None);
    assert_eq!(core.reset_runner_calls(), 1);
}

#[test]
fn reset_discards_resource_stalled_work_before_execution() {
    // Given: a queued copy that cannot acquire a read slot.
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_read_slots(0),
    );
    let queue = submit_copy(&mut core, 0);
    let destination = pdma_model_contract::semantic::BufferId::new(1);
    assert_eq!(
        core.step(),
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::ExecutionResourcePressure
        ))
    );

    // When: reset clears the stalled queue.
    core.reset_runner(WholeDeviceReset::new(DeviceEpoch::initial()))
        .expect("reset");

    // Then: discarded work neither executes nor publishes after reset.
    assert_eq!(core.memory.read_object(destination, 0, 1), Ok(vec![0]));
    assert_eq!(core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(core.irq_snapshot(), None);
    assert!(core.poll_completion(queue).is_err());
}

#[test]
fn reset_discards_pending_retirement_without_late_publication() {
    // Given: two executed copies with the second retained behind publication pressure.
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let queue = core.create_queue(queue_config(2)).expect("queue");
    let source = core.memory.allocate(1).expect("source").object();
    let first_destination = core.memory.allocate(1).expect("first destination").object();
    let second_destination = core
        .memory
        .allocate(1)
        .expect("second destination")
        .object();
    core.memory
        .write_object(source, 0, &[0xa5])
        .expect("source write");
    for (command, destination) in [first_destination, second_destination]
        .into_iter()
        .enumerate()
    {
        core.submit_command(
            queue,
            Command::new(
                CommandId::new(command),
                CommandOperation::Copy(CopyRequest::new(source, destination, 1)),
            ),
        )
        .expect("copy submission");
    }
    core.ring_doorbell(queue).expect("doorbell");
    assert_eq!(core.step(), Ok(ModelStepStatus::Progressed));
    assert_eq!(
        core.step(),
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure
        ))
    );

    // When: reset drops the cached retirement record.
    core.reset_runner(WholeDeviceReset::new(DeviceEpoch::initial()))
        .expect("reset");

    // Then: executed DMA effects persist but no late completion or IRQ can appear.
    assert_eq!(
        core.memory.read_object(first_destination, 0, 1),
        Ok(vec![0xa5])
    );
    assert_eq!(
        core.memory.read_object(second_destination, 0, 1),
        Ok(vec![0xa5])
    );
    assert_eq!(core.pending_dma_result(), None);
    assert_eq!(core.step(), Ok(ModelStepStatus::Idle));
    assert_eq!(core.irq_snapshot(), None);
}
