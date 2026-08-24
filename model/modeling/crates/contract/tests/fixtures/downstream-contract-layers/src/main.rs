use pdma_model_contract::abi::{
    AbiCommand, AbiCopy, AbiId, CopyDmaAddresses, DmaAddress, RingCursor, RingLayout, RingRecord,
};
use pdma_model_contract::bridge::{decode_buffer_id, encode_buffer_id};
use pdma_model_contract::semantic::{BufferId, CopyRequest, DeviceEpoch, EpochQueue, QueueId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = BufferId::new(7);
    let destination = BufferId::new(9);
    let semantic = CopyRequest::new(source, destination, 16);
    let semantic_queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(3));

    let exact = AbiCommand::Copy(AbiCopy::new(
        AbiId::new(41),
        CopyDmaAddresses::new(DmaAddress::new(0x1000), DmaAddress::new(0x2000)),
        16,
    ));
    let mut record = [0_u8; 64];
    exact.encode(&mut record)?;
    let ring = RingLayout::try_new(
        DmaAddress::new(0x4000),
        64,
        1,
        RingRecord::Submission,
    )?;
    let cursor = RingCursor::new(3);

    let encoded = encode_buffer_id(semantic.src())?;
    let roundtrip = decode_buffer_id(encoded)? == semantic.src();
    println!(
        "semantic=independent;abi_record={};bridge=explicit;roundtrip={roundtrip}",
        record.len()
    );
    assert_eq!(ring.slot(cursor), 0);
    assert_eq!(semantic_queue.queue_id(), QueueId::new(3));
    Ok(())
}
