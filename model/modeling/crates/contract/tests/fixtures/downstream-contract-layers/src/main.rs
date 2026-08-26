use pdma_model_contract::abi::{
    AbiCommand, AbiId, DescriptorSchemaPayloadCopyValues, DmaAddress, RingCursor, RingRecord,
};
use pdma_model_contract::binding::{PdmaMemoryRegion, RingLayout};
use pdma_model_contract::semantic::{BufferId, CopyRequest, DeviceEpoch, EpochQueue, QueueId};
use pdma_model_contract::translation::{decode_buffer_id, encode_buffer_id};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = BufferId::new(7);
    let destination = BufferId::new(9);
    let semantic = CopyRequest::new(source, destination, 16);
    let semantic_queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(3));

    let exact = AbiCommand::copy(
        AbiId::new(41),
        DescriptorSchemaPayloadCopyValues {
            source_dma_address: 0x1000,
            destination_dma_address: 0x2000,
            length: 16,
        },
    );
    let mut record = [0_u8; 64];
    exact.encode(&mut record)?;
    let region = PdmaMemoryRegion::try_new(BufferId::new(11), DmaAddress::new(0x4000), 64)?;
    let ring = RingLayout::try_new(region, 1, RingRecord::Submission)?;
    let cursor = RingCursor::new(3);

    let encoded = encode_buffer_id(semantic.src())?;
    let roundtrip = decode_buffer_id(encoded)? == semantic.src();
    println!(
        "semantic=independent;abi_record={};translation=explicit;roundtrip={roundtrip}",
        record.len()
    );
    assert_eq!(ring.slot(cursor), 0);
    assert_eq!(semantic_queue.queue_id(), QueueId::new(3));
    Ok(())
}
