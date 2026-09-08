use super::QueueBlock;
use super::exact::PdmaControlError;
use pdma_model_contract::abi::registers::QueueRegisters;
use pdma_model_contract::abi::{DmaAddress, DmaRegion, QueueRingLayouts, RingCursor, RingLayout};
use pdma_model_contract::semantic::QueueId;

fn layouts() -> QueueRingLayouts {
    QueueRingLayouts::new(
        RingLayout::try_new(
            DmaRegion::try_new(DmaAddress::new(0), 128).expect("SQ region"),
            2,
        )
        .expect("SQ"),
        RingLayout::try_new(
            DmaRegion::try_new(DmaAddress::new(128), 64).expect("CQ region"),
            2,
        )
        .expect("CQ"),
        RingLayout::try_new(
            DmaRegion::try_new(DmaAddress::new(192), 128).expect("EQ region"),
            2,
        )
        .expect("EQ"),
    )
}

#[test]
fn configurations_are_independent_for_each_hardware_queue() {
    // Given: two inactive hardware queue instances.
    let mut queues = QueueBlock::default();
    queues
        .write_configuration(0, QueueRegisters::SQ_BASE.offset(), 0x1000)
        .expect("first queue");

    // When: software configures another queue's base address.
    queues
        .write_configuration(1, QueueRegisters::SQ_BASE.offset(), 0x3000)
        .expect("other queue");

    // Then: each queue retains its own configuration.
    assert_eq!(
        queues.read_direct(0, QueueRegisters::SQ_BASE.offset()),
        Some(0x1000)
    );
    assert_eq!(
        queues.read_direct(1, QueueRegisters::SQ_BASE.offset()),
        Some(0x3000)
    );
    assert_eq!(
        queues.read_direct(2, QueueRegisters::SQ_BASE.offset()),
        Some(0)
    );
}

#[test]
fn queue_configuration_grows_beyond_sixty_four_instances() {
    // Given: an empty hardware queue block.
    let mut queues = QueueBlock::default();

    // When: software configures more than sixty-four queue instances.
    for index in 0..65 {
        queues
            .write_configuration(
                index,
                QueueRegisters::SQ_BASE.offset(),
                u64::from(index) * 128,
            )
            .expect("config");
    }

    // Then: the final instance retains its own configuration.
    assert_eq!(queues.queue_count(), 65);
    assert_eq!(
        queues.read_direct(64, QueueRegisters::SQ_BASE.offset()),
        Some(8192)
    );
}

#[test]
fn active_queue_configuration_rejection_preserves_geometry() {
    // Given: two configured slots with the second activated first.
    let mut queues = QueueBlock::default();
    queues
        .write_configuration(0, QueueRegisters::SQ_BASE.offset(), 4096)
        .expect("first");
    queues
        .write_configuration(1, QueueRegisters::SQ_BASE.offset(), 0)
        .expect("second");
    queues
        .install_exact(QueueId::new(1), layouts())
        .expect("activate second");

    // When: software tries to change the active slot's geometry.
    let result = queues.write_configuration(1, QueueRegisters::SQ_BASE.offset(), 8192);

    // Then: the active geometry remains immutable and the first slot remains inactive.
    assert_eq!(result, Err(PdmaControlError::InvalidTransaction));
    assert_eq!(
        queues.read_direct(1, QueueRegisters::SQ_BASE.offset()),
        Some(0)
    );
    assert!(!queues.is_active(0));
}

#[test]
fn submission_publication_does_not_capture_without_doorbell() {
    // Given: an active queue with empty rings.
    let mut queues = QueueBlock::default();
    queues
        .write_configuration(0, QueueRegisters::SQ_BASE.offset(), 0)
        .expect("config");
    queues
        .install_exact(QueueId::new(0), layouts())
        .expect("activate");

    // When: software publishes a submission producer.
    queues
        .exact_ring_mut(QueueId::new(0))
        .expect("queue")
        .publish_submission_producer(RingCursor::new(1))
        .expect("publish");

    // Then: the generated register image exposes the producer with capture still zero.
    assert_eq!(
        queues.read_direct(0, QueueRegisters::SQ_PRODUCER.offset()),
        Some(1)
    );
    assert_eq!(
        queues.read_direct(0, QueueRegisters::SQ_CAPTURED.offset()),
        Some(0)
    );
}

#[test]
fn invalid_capture_preserves_published_and_captured_cursors() {
    // Given: an active queue with one published submission.
    let mut queues = QueueBlock::default();
    queues
        .write_configuration(0, QueueRegisters::SQ_BASE.offset(), 0)
        .expect("config");
    queues
        .install_exact(QueueId::new(0), layouts())
        .expect("activate");
    queues
        .exact_ring_mut(QueueId::new(0))
        .expect("queue")
        .publish_submission_producer(RingCursor::new(1))
        .expect("publish");

    // When: software captures beyond the published producer.
    let result = queues
        .exact_ring_mut(QueueId::new(0))
        .expect("queue")
        .capture_submission_producer(RingCursor::new(2));

    // Then: rejection preserves both cursors.
    assert_eq!(result, Err(PdmaControlError::InvalidTransaction));
    assert_eq!(
        queues.read_direct(0, QueueRegisters::SQ_PRODUCER.offset()),
        Some(1)
    );
    assert_eq!(
        queues.read_direct(0, QueueRegisters::SQ_CAPTURED.offset()),
        Some(0)
    );
}
