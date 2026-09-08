use crate::device::ModelStepStatus;
use crate::test_support::{TestQueue, configure, copy, doorbell, read_queue, submit};
use crate::{AsyncStallReason, DeviceCore, PdmaMemoryProvisioning};
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::registers::QueueRegisters;
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::Arc;

fn fixture(resources: ResourceConfig, faulting: bool) -> (DeviceCore, TestQueue) {
    let mut provisioning = PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new()));
    let mut core = DeviceCore::new(provisioning.memory_access(), resources);
    let queue = configure(&mut core, &mut provisioning, 0, [4, 4, 4]);
    let source = provisioning.allocate(&mut core.memory, 4).expect("source");
    let destination = provisioning
        .allocate(&mut core.memory, 4)
        .expect("destination");
    core.memory
        .write_memory(source.base(), &[1, 2, 3, 4])
        .expect("source bytes");
    let source_address = if faulting {
        u64::MAX - 4
    } else {
        source.base().raw()
    };
    for id in 0..2 {
        submit(
            &mut core,
            queue,
            copy(id, source_address, destination.base().raw(), 4),
        );
    }
    doorbell(&mut core, queue);
    (core, queue)
}

#[test]
fn completion_zero_capacity_stalls_with_an_empty_dma_ring() {
    // Given: a runnable copy and a four-entry CQ with zero publication credits.
    let (mut core, queue) = fixture(
        ResourceConfig::default().with_completion_publication_slots(0),
        false,
    );

    // When: one command executes and reaches retirement.
    let status = core.step().expect("service request");

    // Then: the result is retained without publishing a completion.
    assert_eq!(
        status,
        ModelStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::CQ_PRODUCER.offset()),
        0
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::SQ_CONSUMER.offset()),
        1
    );
}

#[test]
fn completion_one_capacity_stalls_before_the_dma_ring_is_full() {
    // Given: one unread completion occupies the sole publication credit in a four-entry CQ.
    let (mut core, queue) = fixture(
        ResourceConfig::default().with_completion_publication_slots(1),
        false,
    );
    assert_eq!(
        core.step().expect("first command"),
        ModelStepStatus::Progressed
    );

    // When: the second command reaches retirement.
    let status = core.step().expect("second command");

    // Then: the existing completion remains the only published record.
    assert_eq!(
        status,
        ModelStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::CQ_PRODUCER.offset()),
        1
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::SQ_CONSUMER.offset()),
        2
    );
}

#[test]
fn event_zero_capacity_stalls_with_an_empty_dma_ring() {
    // Given: a faulting DMA operand and a four-entry EQ with zero publication credits.
    let (mut core, queue) = fixture(
        ResourceConfig::default().with_event_publication_slots(0),
        true,
    );
    // When: execution faults and attempts event publication.
    let status = core.step().expect("retirement request");

    // Then: no event is published while the fault remains retained.
    assert_eq!(
        status,
        ModelStepStatus::Stalled(AsyncStallReason::EventPublicationPressure)
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::EQ_PRODUCER.offset()),
        0
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::CQ_PRODUCER.offset()),
        0
    );
}

#[test]
fn event_one_capacity_stalls_before_the_dma_ring_is_full() {
    // Given: one unread fault occupies the sole publication credit in a four-entry EQ.
    let (mut core, queue) = fixture(
        ResourceConfig::default().with_event_publication_slots(1),
        true,
    );
    assert!(core.step().is_err());

    // When: a later request attempts to publish the second fault.
    let status = core.step().expect("retirement request");

    // Then: the first event remains the only published record.
    assert_eq!(
        status,
        ModelStepStatus::Stalled(AsyncStallReason::EventPublicationPressure)
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::EQ_PRODUCER.offset()),
        1
    );
    assert_eq!(
        read_queue(&core, queue, QueueRegisters::CQ_PRODUCER.offset()),
        0
    );
}
