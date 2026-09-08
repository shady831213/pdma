use crate::queue::QueueBlock;
use crate::queue::exact::PreparedExactQueue;
use pdma_model_contract::abi::mmio::{MmioResult, QueueCommand};
use pdma_model_contract::abi::{DmaAddress, DmaRegion, QueueRingLayouts, RingLayout};
use pdma_model_contract::semantic::{
    BufferId, DeviceEpoch, EventRouting, QueueActivation, QueueConfig, QueueId, QueueRingObjects,
    SoftwareOwnerId,
};

fn active_queue() -> QueueBlock {
    let activation = QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        QueueRingObjects::new(BufferId::new(0), BufferId::new(1), BufferId::new(2)),
    );
    let layouts = QueueRingLayouts::new(
        RingLayout::try_new(
            DmaRegion::try_new(DmaAddress::new(0), 128).expect("SQ region"),
            2,
        )
        .expect("SQ layout"),
        RingLayout::try_new(
            DmaRegion::try_new(DmaAddress::new(128), 64).expect("CQ region"),
            2,
        )
        .expect("CQ layout"),
        RingLayout::try_new(
            DmaRegion::try_new(DmaAddress::new(192), 128).expect("EQ region"),
            2,
        )
        .expect("EQ layout"),
    );
    let mut queues = QueueBlock::default();
    let prepared = queues
        .prepare_exact(PreparedExactQueue::new(activation, layouts))
        .expect("prepare queue");
    queues.install_exact(DeviceEpoch::initial(), prepared);
    queues
}

#[test]
fn owner_mmio_publishes_submission_without_device_core() {
    // Given: a real active queue owner with staged producer publication.
    let mut queues = active_queue();
    queues.registers.command = QueueCommand::PublishSqProducer.code().into();
    queues.registers.new_cursor = 1.into();

    // When: the queue owner executes its own register command.
    let effect = queues.execute_mmio(DeviceEpoch::initial());

    // Then: the owner publishes the producer without requesting runnable capture.
    assert_eq!(effect, (true, None));
    assert_eq!(queues.registers.result.raw(), MmioResult::Ok.code());
    assert_eq!(queues.registers.response.sq_producer.raw(), 1);
    assert_eq!(queues.registers.response.sq_captured.raw(), 0);
    assert_eq!(
        queues
            .exact_ring(QueueId::new(0))
            .expect("active queue")
            .status()
            .expect("queue status")
            .submission()
            .producer()
            .raw(),
        1,
    );
}

#[test]
fn owner_mmio_capture_reports_runnable_queue_even_when_cursor_is_unchanged() {
    for captured in [0, 1] {
        // Given: a published producer with either a fresh or repeated capture request.
        let mut queues = active_queue();
        let active = queues
            .exact_ring_mut(QueueId::new(0))
            .expect("active queue");
        active
            .publish_submission_producer(pdma_model_contract::abi::RingCursor::new(1))
            .expect("publish producer");
        active
            .capture_submission_producer(pdma_model_contract::abi::RingCursor::new(captured))
            .expect("initial capture");
        queues.registers.command = QueueCommand::CaptureSqProducer.code().into();
        queues.registers.expected_cursor = captured.into();
        queues.registers.new_cursor = 1.into();

        // When: the queue owner accepts capture of the published producer.
        let effect = queues.execute_mmio(DeviceEpoch::initial());

        // Then: both captures request runnable admission, while only fresh capture changes state.
        assert_eq!(effect, (captured == 0, Some(QueueId::new(0))));
        assert_eq!(queues.registers.result.raw(), MmioResult::Ok.code());
        assert_eq!(queues.registers.response.sq_captured.raw(), 1);
    }
}

#[test]
fn owner_mmio_rejections_preserve_response_and_queue_cursor() {
    for (command, expected_epoch, expected_cursor, result) in [
        (u32::MAX, 0, 0, MmioResult::UnknownCommand),
        (
            QueueCommand::CaptureSqProducer.code(),
            1,
            0,
            MmioResult::StaleEpoch,
        ),
        (
            QueueCommand::PublishSqProducer.code(),
            0,
            1,
            MmioResult::InvalidTransaction,
        ),
        (
            QueueCommand::CaptureSqProducer.code(),
            0,
            0,
            MmioResult::InvalidTransaction,
        ),
    ] {
        // Given: an active queue with a saved response and an invalid staged request.
        let mut queues = active_queue();
        queues.registers.response.sq_producer = 7.into();
        let response = queues.registers.response;
        queues.registers.command = command.into();
        queues.registers.expected_epoch = expected_epoch.into();
        queues.registers.expected_cursor = expected_cursor.into();
        queues.registers.new_cursor = 1.into();

        // When: the queue owner rejects the staged command.
        let effect = queues.execute_mmio(DeviceEpoch::initial());

        // Then: no runnable admission, response replacement, or queue mutation occurs.
        assert_eq!(effect, (false, None));
        assert_eq!(queues.registers.result.raw(), result.code());
        assert_eq!(queues.registers.response, response);
        let status = queues
            .exact_ring(QueueId::new(0))
            .expect("active queue")
            .status()
            .expect("status");
        assert_eq!(status.submission().producer().raw(), 0);
        assert_eq!(status.captured_submission_producer().raw(), 0);
    }
}
