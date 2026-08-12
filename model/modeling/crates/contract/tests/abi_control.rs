//! ABI-v1 ring arithmetic and typed control vocabulary.

use pdma_model_contract::{
    AbiAddressError, AbiControlError, AbiDiscovery, AbiLimits, ControlRequest, DeviceEpoch,
    DeviceRunState, DeviceStatus, DiscoveryRequest, DmaAddress, DmaRegion, EpochIrqRoute,
    EpochIrqSource, IrqControl, QueueActivation, QueueControl, QueueRingRegions, RingCursor,
    RingCursors, RingLayout, RingRecord, WholeDeviceReset,
};
use pdma_model_contract::{
    BufferId, EventRouting, IrqRoute, IrqSource, IrqVector, QueueConfig, QueueId, SoftwareOwnerId,
};

const fn region(id: usize, base: u64, length: u64) -> Result<DmaRegion, AbiAddressError> {
    DmaRegion::try_new(BufferId::new(id), DmaAddress::new(base), length)
}

fn assert_layout_errors(actual: [Option<AbiControlError>; 5]) {
    assert_eq!(
        actual,
        [
            Some(AbiControlError::InvalidRingDepth {
                depth: 0,
                maximum: u32::MAX,
            }),
            Some(AbiControlError::InvalidRingDepth {
                depth: u64::from(u32::MAX) + 1,
                maximum: u32::MAX,
            }),
            Some(AbiControlError::RingExtentOverflow {
                depth: u64::MAX,
                record_size: 64,
            }),
            Some(AbiControlError::MisalignedRing {
                base: 0x3_0001,
                alignment: 64,
            }),
            Some(AbiControlError::RingExtentMismatch {
                expected: 64,
                actual: 63,
            }),
        ]
    );
}

fn assert_cursor_errors(actual: [Option<AbiControlError>; 5]) {
    assert_eq!(
        actual,
        [
            Some(AbiControlError::CursorRegression {
                producer: 0,
                consumer: 1,
            }),
            Some(AbiControlError::CursorDistanceExceeded {
                producer: 2,
                consumer: 0,
                depth: 1,
            }),
            Some(AbiControlError::CursorIncrementOverflow {
                cursor: u64::MAX,
                increment: 1,
            }),
            Some(AbiControlError::CursorUpdateRegression {
                current: 1,
                proposed: 0,
            }),
            Some(AbiControlError::CursorRegression {
                producer: 1,
                consumer: 2,
            }),
        ]
    );
}

fn assert_control_errors(actual: [Option<AbiControlError>; 4]) {
    assert_eq!(
        actual,
        [
            Some(AbiControlError::EventDepthMismatch {
                expected: 1,
                actual: 2,
            }),
            Some(AbiControlError::DeviceEpochOverflow { current: u64::MAX }),
            Some(AbiControlError::UnknownStatusValue { value: u16::MAX }),
            Some(AbiControlError::UnknownFeatureBits { bits: 1_u64 << 63 }),
        ]
    );
}

#[test]
fn ring_and_control_vocabulary_follow_v1_contract() {
    // Given: three independently sized and aligned queue-local ABI-v1 ring regions.
    let region = |id, base, length| {
        region(id, base, length).expect("test region has a representable extent")
    };
    let config = QueueConfig::new(3, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(9));
    let submission = RingLayout::try_new(region(1, 0x1_0000, 192), 3, RingRecord::Submission)
        .expect("three descriptors fit exactly");
    let completion = RingLayout::try_new(region(2, 0x2_0020, 64), 2, RingRecord::Completion)
        .expect("two completions fit exactly");
    let event = RingLayout::try_new(region(3, 0x3_0000, 128), 2, RingRecord::Event)
        .expect("two events fit exactly");
    let maximum_extent = u64::from(u32::MAX) * 64;
    let maximum = RingLayout::try_new(
        region(4, 0x4_0000, maximum_extent),
        u64::from(u32::MAX),
        RingRecord::Submission,
    )
    .expect("the advertised maximum depth remains representable");
    let cursors = RingCursors::try_new(submission, RingCursor::new(9), RingCursor::new(6))
        .expect("distance equal to depth retains the final slot");

    // When: software advances monotonic shadows and builds typed control transactions.
    let idempotent = cursors
        .update_consumer(RingCursor::new(6))
        .expect("idempotent consumer publication is valid");
    let drained = idempotent
        .update_consumer(RingCursor::new(9))
        .expect("forward consumer publication is valid");
    let wrapped = RingCursor::new(10);
    let produced = drained
        .update_producer(wrapped)
        .expect("forward producer publication is valid")
        .update_producer(wrapped)
        .expect("idempotent producer publication is valid");
    let activation = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(4),
        config,
        QueueRingRegions::new(submission, completion, event),
    )
    .expect("canonical queue-local event depth is valid");
    let irq_source = EpochIrqSource::new(DeviceEpoch::initial(), IrqSource::Event(QueueId::new(4)));
    let irq_route = EpochIrqRoute::new(irq_source, IrqRoute::Message(IrqVector::new(31)));
    let requests = [
        ControlRequest::Discover(
            DiscoveryRequest::try_new(1, 0xff).expect("v1 and its complete feature mask are valid"),
        ),
        ControlRequest::ReadLimits,
        ControlRequest::ReadStatus,
        ControlRequest::ActivateQueue(activation),
        ControlRequest::Queue(QueueControl::publish_submission_producer(
            DeviceEpoch::initial(),
            QueueId::new(4),
            wrapped,
        )),
        ControlRequest::Irq(IrqControl::ConfigureRoute(irq_route)),
        ControlRequest::Reset(WholeDeviceReset::new(DeviceEpoch::initial())),
    ];
    let discovery = AbiDiscovery::v1();
    let limits = AbiLimits::v1();
    let status = DeviceStatus::new(DeviceEpoch::initial(), DeviceRunState::Idle);

    // Then: extents, capacity, wrapping, derived depth, epochs, and typed requests are exact.
    assert_eq!(
        [submission.extent(), completion.extent(), event.extent()],
        [192, 64, 128]
    );
    assert_eq!(
        [submission.depth(), completion.depth(), event.depth()],
        [3, 2, 2]
    );
    assert_eq!(config.event_depth(), 2);
    assert_eq!(maximum.depth(), u32::MAX);
    assert_eq!(maximum.extent(), maximum_extent);
    assert!(cursors.is_full());
    assert_eq!(cursors.available(), 0);
    assert_eq!(drained.available(), 3);
    assert_eq!(submission.slot(RingCursor::new(9)), 0);
    assert_eq!(submission.slot(wrapped), 1);
    assert_eq!(produced.producer(), wrapped);
    assert_eq!(wrapped.checked_increment(5), Ok(RingCursor::new(15)));
    assert_eq!(
        activation.rings(),
        QueueRingRegions::new(submission, completion, event)
    );
    assert_eq!(irq_route.source().epoch(), DeviceEpoch::initial());
    assert_eq!(discovery.version(), 1);
    assert_eq!(discovery.features().bits(), 0xff);
    assert_eq!(limits.max_ring_depth(), u32::MAX);
    assert_eq!(status.run_state(), DeviceRunState::Idle);
    assert_eq!(requests.len(), 7);
    println!(
        "rings: extents={:?} full={} transitions=({}->{}) slots=({}, {}) epoch={} controls={}",
        [submission.extent(), completion.extent(), event.extent()],
        cursors.is_full(),
        cursors.consumer().raw(),
        drained.consumer().raw(),
        submission.slot(RingCursor::new(9)),
        submission.slot(wrapped),
        irq_route.source().epoch().raw(),
        requests.len(),
    );
}

#[test]
fn invalid_cursor_layout_event_depth_epoch_and_control_values_are_rejected() {
    // Given: malformed depths, regions, shadows, epochs, and untrusted control values.
    let region = |id, base, length| {
        region(id, base, length).expect("test region has a representable extent")
    };
    let depth_zero = RingLayout::try_new(region(10, 0x1_0000, 0), 0, RingRecord::Submission);
    let depth_oversized = RingLayout::try_new(
        region(11, 0x2_0000, 64),
        u64::from(u32::MAX) + 1,
        RingRecord::Submission,
    );
    let extent_overflow =
        RingLayout::checked_extent(u64::MAX, RingRecord::Submission.record_size());
    let misaligned = RingLayout::try_new(region(12, 0x3_0001, 64), 1, RingRecord::Submission);
    let wrong_extent = RingLayout::try_new(region(13, 0x4_0000, 63), 1, RingRecord::Submission);
    let one = RingLayout::try_new(region(14, 0x5_0000, 64), 1, RingRecord::Submission)
        .expect("depth-one full-capacity fixture is valid");
    let regression = RingCursors::try_new(one, RingCursor::new(0), RingCursor::new(1));
    let overrun = RingCursors::try_new(one, RingCursor::new(2), RingCursor::new(0));
    let full = RingCursors::try_new(one, RingCursor::new(1), RingCursor::new(0))
        .expect("depth-one full fixture is valid");
    let update_regression = full.update_producer(RingCursor::new(0));
    let subtraction_regression = full.update_consumer(RingCursor::new(2));
    let cursor_overflow = RingCursor::new(u64::MAX).checked_increment(1);
    let config = QueueConfig::new(1, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(5));
    let submission = one;
    let completion = RingLayout::try_new(region(15, 0x6_0000, 32), 1, RingRecord::Completion)
        .expect("completion fixture is valid");
    let mismatched_event = RingLayout::try_new(region(16, 0x7_0000, 128), 2, RingRecord::Event)
        .expect("event ring itself is valid");

    // When: every value crosses its checked typed boundary.
    let activation = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        config,
        QueueRingRegions::new(submission, completion, mismatched_event),
    );
    let epoch_overflow = DeviceEpoch::new(u64::MAX).next();
    let unknown_status = DeviceRunState::try_from(u16::MAX);
    let unknown_features = DiscoveryRequest::try_new(1, 1_u64 << 63);

    // Then: each exact failure is typed, and immutable input values remain unchanged.
    assert_layout_errors([
        depth_zero.err(),
        depth_oversized.err(),
        extent_overflow.err(),
        misaligned.err(),
        wrong_extent.err(),
    ]);
    assert_cursor_errors([
        regression.err(),
        overrun.err(),
        cursor_overflow.err(),
        update_regression.err(),
        subtraction_regression.err(),
    ]);
    assert_control_errors([
        activation.err(),
        epoch_overflow.err(),
        unknown_status.err(),
        unknown_features.err(),
    ]);
    assert_eq!(one.depth(), 1);
    assert_eq!(config.event_depth(), 1);
    println!(
        "typed errors: depth={depth_zero:?} extent={extent_overflow:?} regression={regression:?} overrun={overrun:?} epoch={epoch_overflow:?} status={unknown_status:?}"
    );
}
