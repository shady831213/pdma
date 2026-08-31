//! Independent ABI-layout and semantic queue-control proofs.

use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiControlError, AbiDiscovery, AbiEvent, AbiLimits,
    DiscoveryRequest, DmaAddress, DmaRegion, RingCursor as AbiRingCursor, RingLayout,
    RingState as AbiRingState,
};
use pdma_model_contract::semantic::{
    BufferId, DeviceEpoch, EventRouting, QueueActivation, QueueConfig, QueueControl, QueueId,
    QueueRingObjects, RingCursor, RingCursors, SemanticError, SoftwareOwnerId,
};

const fn region(
    base: u64,
    length: u64,
) -> Result<DmaRegion, pdma_model_contract::abi::AbiAddressError> {
    DmaRegion::try_new(DmaAddress::new(base), length)
}

#[test]
fn abi_layout_and_semantic_control_have_independent_owners() {
    // Given: exact ABI ring extents and separate semantic DMA-object assignments.
    let submission = RingLayout::<AbiCommand>::try_new(
        region(0x1_0000, 192).expect("test region has a representable extent"),
        3,
    )
    .expect("three descriptors fit exactly");
    let completion = RingLayout::<AbiCompletion>::try_new(
        region(0x2_0020, 64).expect("test region has a representable extent"),
        2,
    )
    .expect("two completions fit exactly");
    let event = RingLayout::<AbiEvent>::try_new(
        region(0x3_0000, 128).expect("test region has a representable extent"),
        2,
    )
    .expect("two events fit exactly");
    let config = QueueConfig::new(3, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(9));
    let objects = QueueRingObjects::new(BufferId::new(1), BufferId::new(2), BufferId::new(3));
    let activation = QueueActivation::new(DeviceEpoch::initial(), QueueId::new(4), config, objects);
    let cursors = RingCursors::try_new(3, RingCursor::new(9), RingCursor::new(6))
        .expect("distance equal to depth retains the final slot");
    let abi_state = AbiRingState::try_new(submission, AbiRingCursor::new(9), AbiRingCursor::new(6))
        .expect("ABI cursors retain exactly one full ring");

    // When: semantic cursors advance and one typed queue control is created.
    let drained = cursors
        .update_consumer(RingCursor::new(9))
        .expect("forward consumer publication is valid");
    let wrapped = RingCursor::new(10);
    let produced = drained
        .update_producer(wrapped)
        .expect("forward producer publication is valid");
    let control =
        QueueControl::publish_submission_producer(DeviceEpoch::initial(), QueueId::new(4), wrapped);

    // Then: ABI byte geometry and semantic identity/state remain independently exact.
    assert_eq!(
        [submission.extent(), completion.extent(), event.extent()],
        [192, 64, 128]
    );
    assert_eq!(submission.slot(AbiRingCursor::new(9)), 0);
    assert_eq!(submission.slot(AbiRingCursor::new(wrapped.raw())), 1);
    assert_eq!(
        submission.entry_address(AbiRingCursor::new(wrapped.raw())),
        Ok(DmaAddress::new(0x1_0040))
    );
    assert!(cursors.is_full());
    assert!(abi_state.is_full());
    assert_eq!(abi_state.layout(), submission);
    assert_eq!(abi_state.producer(), AbiRingCursor::new(9));
    assert_eq!(abi_state.consumer(), AbiRingCursor::new(6));
    assert_eq!(drained.available(), 3);
    assert_eq!(produced.producer(), wrapped);
    assert_eq!(activation.rings(), objects);
    assert_eq!(activation.config(), config);
    assert!(matches!(
        control,
        QueueControl::PublishSubmissionProducer(update)
            if update.queue() == activation.queue() && update.cursor() == wrapped
    ));
}

#[test]
fn malformed_abi_layout_and_semantic_state_report_their_own_errors() {
    // Given: malformed ABI geometry and malformed semantic cursor transitions.
    let depth_zero = RingLayout::<AbiCommand>::try_new(
        region(0x1_0000, 0).expect("test region has a representable extent"),
        0,
    );
    let depth_oversized = RingLayout::<AbiCommand>::try_new(
        region(0x2_0000, 64).expect("test region has a representable extent"),
        u64::from(u32::MAX) + 1,
    );
    let extent_overflow = RingLayout::<AbiCommand>::checked_extent(u64::MAX);
    let misaligned = RingLayout::<AbiCommand>::try_new(
        region(0x3_0001, 64).expect("test region has a representable extent"),
        1,
    );
    let wrong_extent = RingLayout::<AbiCommand>::try_new(
        region(0x4_0000, 63).expect("test region has a representable extent"),
        1,
    );
    let regression = RingCursors::try_new(1, RingCursor::new(0), RingCursor::new(1));
    let overrun = RingCursors::try_new(1, RingCursor::new(2), RingCursor::new(0));
    let full = RingCursors::try_new(1, RingCursor::new(1), RingCursor::new(0))
        .expect("depth-one full fixture is valid");
    let update_regression = full.update_producer(RingCursor::new(0));
    let subtraction_regression = full.update_consumer(RingCursor::new(2));
    let cursor_overflow = RingCursor::new(u64::MAX).checked_increment(1);
    let epoch_overflow = DeviceEpoch::new(u64::MAX).next();

    // Then: each layer returns only errors owned by that layer.
    assert_eq!(
        [
            depth_zero.err(),
            depth_oversized.err(),
            extent_overflow.err(),
            misaligned.err(),
            wrong_extent.err(),
        ],
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
    assert_eq!(
        [
            regression.err(),
            overrun.err(),
            cursor_overflow.err(),
            update_regression.err(),
            subtraction_regression.err(),
            epoch_overflow.err(),
        ],
        [
            Some(SemanticError::CursorRegression {
                producer: 0,
                consumer: 1,
            }),
            Some(SemanticError::CursorDistanceExceeded {
                producer: 2,
                consumer: 0,
                depth: 1,
            }),
            Some(SemanticError::CursorIncrementOverflow {
                cursor: u64::MAX,
                increment: 1,
            }),
            Some(SemanticError::CursorUpdateRegression {
                current: 1,
                proposed: 0,
            }),
            Some(SemanticError::CursorRegression {
                producer: 1,
                consumer: 2,
            }),
            Some(SemanticError::DeviceEpochOverflow { current: u64::MAX }),
        ]
    );
}

#[test]
fn malformed_abi_cursors_report_abi_owned_errors() {
    // Given: one exact depth-one ABI ring and three malformed cursor operations.
    let layout = RingLayout::<AbiCommand>::try_new(
        region(0x5_0000, 64).expect("test region has a representable extent"),
        1,
    )
    .expect("one ABI descriptor has an exact ring extent");

    // When: ordering, retained capacity, and fixed-width advancement are violated.
    let regression = AbiRingState::try_new(layout, AbiRingCursor::new(0), AbiRingCursor::new(1));
    let overrun = AbiRingState::try_new(layout, AbiRingCursor::new(2), AbiRingCursor::new(0));
    let overflow = AbiRingCursor::new(u64::MAX).checked_increment(1);

    // Then: only ABI-owned cursor errors cross the exact control boundary.
    assert_eq!(
        [regression.err(), overrun.err(), overflow.err()],
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
        ]
    );
}

#[test]
fn discovery_and_limits_remain_exact_abi_values() {
    // Given/When: ABI discovery accepts the complete v1 feature mask.
    let request = DiscoveryRequest::try_new(1, 0xff);
    let discovery = AbiDiscovery::v1();
    let limits = AbiLimits::v1();

    // Then: numeric ABI negotiation and limits remain unchanged.
    assert!(request.is_ok());
    assert_eq!(discovery.version(), 1);
    assert_eq!(discovery.features().bits(), 0xff);
    assert_eq!(limits.max_ring_depth(), u32::MAX);
    assert_eq!(
        DiscoveryRequest::try_new(1, 1_u64 << 63),
        Err(AbiControlError::UnknownFeatureBits { bits: 1_u64 << 63 })
    );
}
