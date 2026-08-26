//! Standalone downstream layer-boundary behavior.

use pdma_model_contract::abi::{{
        AbiCommand, AbiId, DescriptorSchemaPayloadCopyValues, DmaAddress, RingCursor, RingRecord,
    }};
use pdma_model_contract::binding::{{PdmaMemoryRegion, RingLayout}};
use pdma_model_contract::semantic::{{BufferId, CopyRequest, DeviceEpoch, EpochQueue, QueueId}};
use pdma_model_contract::translation::{{decode_buffer_id, encode_buffer_id}};

#[test]
fn independent_values_compose_only_through_translation() {
    // Given: unrelated semantic and exact ABI values.
    let request = CopyRequest::new(BufferId::new(7), BufferId::new(9), 16);
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(3));
    let exact = AbiCommand::copy(
        AbiId::new(41),
        DescriptorSchemaPayloadCopyValues {{
            source_dma_address: 0x1000,
            destination_dma_address: 0x2000,
            length: 16,
        }},
    );

    // When: the record is encoded and the semantic ID crosses the translation.
    let mut bytes = [0_u8; 64];
    let encoded = exact.encode(&mut bytes);
    let roundtrip = encode_buffer_id(request.src()).map(decode_buffer_id);
    let region = PdmaMemoryRegion::try_new(BufferId::new(11), DmaAddress::new(0x4000), 64)
        .expect("exact region must fit");
    let ring = RingLayout::try_new(region, 1, RingRecord::Submission);

    // Then: both real values and the explicit translation preserve their contracts.
    assert_eq!(encoded, Ok(()));
    assert_eq!(bytes.len(), 64);
    assert_eq!(roundtrip, Ok(Ok(request.src())));
    assert_eq!(queue.queue_id(), QueueId::new(3));
    assert_eq!(ring.map(|layout| layout.slot(RingCursor::new(3))), Ok(0));
}
