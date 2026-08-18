//! Standalone downstream layer-boundary behavior.

use pdma_model_contract::abi::{{
        AbiCommand, AbiCopy, AbiId, CopyIovas, DmaAddress, RingCursor, RingLayout, RingRecord,
    }};
use pdma_model_contract::bridge::{{decode_buffer_id, encode_buffer_id}};
use pdma_model_contract::semantic::{{BufferId, CopyRequest, DeviceEpoch, EpochQueue, QueueId}};

#[test]
fn independent_values_compose_only_through_bridge() {
    // Given: unrelated semantic and exact ABI values.
    let request = CopyRequest::new(BufferId::new(7), BufferId::new(9), 16);
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(3));
    let exact = AbiCommand::Copy(AbiCopy::new(
        AbiId::new(41),
        CopyIovas::new(DmaAddress::new(0x1000), DmaAddress::new(0x2000)),
        16,
    ));

    // When: the record is encoded and the semantic ID crosses the bridge.
    let mut bytes = [0_u8; 64];
    let encoded = exact.encode(&mut bytes);
    let roundtrip = encode_buffer_id(request.src()).map(decode_buffer_id);
    let ring = RingLayout::try_new(
        DmaAddress::new(0x4000),
        64,
        1,
        RingRecord::Submission,
    );

    // Then: both real values and the explicit bridge preserve their contracts.
    assert_eq!(encoded, Ok(()));
    assert_eq!(bytes.len(), 64);
    assert_eq!(roundtrip, Ok(Ok(request.src())));
    assert_eq!(queue.queue_id(), QueueId::new(3));
    assert_eq!(ring.map(|layout| layout.slot(RingCursor::new(3))), Ok(0));
}
