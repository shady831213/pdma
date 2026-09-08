use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, DmaAddress, DmaRegion,
    RingCursor as AbiRingCursor, RingLayout,
};
use pdma_model_contract::semantic::{BufferId, DeviceEpoch, EpochQueue, QueueId, RingCursor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = BufferId::new(7);
    let semantic_queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(3));

    let exact = AbiCommand::from_payload(
        41,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: 0x1000,
            destination_dma_address: 0x2000,
            length: 16.into(),
        }),
    );
    let mut record = [0_u8; 64];
    exact.encode(&mut record)?;
    let region = DmaRegion::try_new(DmaAddress::new(0x4000), 64)?;
    let ring = RingLayout::<AbiCommand>::try_new(region, 1)?;
    let cursor = RingCursor::new(3);

    let encoded = u64::try_from(source.raw())?;
    let roundtrip = BufferId::new(usize::try_from(encoded)?) == source;
    println!(
        "semantic=independent;abi_record={};conversion=owned;roundtrip={roundtrip}",
        record.len()
    );
    assert_eq!(ring.slot(AbiRingCursor::new(cursor.raw())), 0);
    assert_eq!(semantic_queue.queue_id(), QueueId::new(3));
    Ok(())
}
