//! Standalone downstream layer-boundary behavior.

use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, DmaAddress, DmaRegion,
    RingCursor as AbiRingCursor, RingLayout,
};
use pdma_model_contract::semantic::{BufferId, DeviceEpoch, EpochQueue, QueueId};

#[test]
fn independent_values_compose_only_through_owner_conversions() {
    // Given: unrelated semantic and exact ABI values.
    let source = BufferId::new(7);
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(3));
    let exact = AbiCommand::from_payload(
        41,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: 0x1000,
            destination_dma_address: 0x2000,
            length: 16.into(),
        }),
    );

    // When: the record is encoded and the semantic ID uses its owning conversions.
    let mut bytes = [0_u8; 64];
    let encoded = exact.encode(&mut bytes);
    let roundtrip = u64::try_from(source.raw()).map(|id| usize::try_from(id).map(BufferId::new));
    let region = DmaRegion::try_new(DmaAddress::new(0x4000), 64).expect("exact region must fit");
    let ring = RingLayout::<AbiCommand>::try_new(region, 1);

    // Then: both real values and the owning conversions preserve their contracts.
    assert_eq!(encoded, Ok(()));
    assert_eq!(bytes.len(), 64);
    assert_eq!(roundtrip, Ok(Ok(source)));
    assert_eq!(queue.queue_id(), QueueId::new(3));
    assert_eq!(ring.map(|layout| layout.slot(AbiRingCursor::new(3))), Ok(0));
}
