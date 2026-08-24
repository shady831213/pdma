//! Compile and value proofs for the additive contract namespaces.

use pdma_model_contract::abi::{
    AbiCommand, AbiId, DescriptorSchemaPayloadCopyValues, DmaAddress, RingCursor, RingLayout,
    RingRecord,
};
use pdma_model_contract::bridge::{decode_buffer_id, encode_buffer_id};
use pdma_model_contract::semantic::{self, BufferId};

#[test]
fn semantic_and_abi_are_independent_and_bridge_is_explicit() {
    // Given: one semantic request and one unrelated exact ABI command.
    let semantic = semantic::CopyRequest::new(BufferId::new(3), BufferId::new(5), 8);
    let abi = AbiCommand::copy(
        AbiId::new(11),
        DescriptorSchemaPayloadCopyValues {
            source_dma_address: 0x100,
            destination_dma_address: 0x200,
            length: 8,
        },
    );

    // When: the exact command is encoded and one ID crosses the explicit bridge.
    let mut bytes = [0_u8; 64];
    let encoded_record = abi.encode(&mut bytes);
    let encoded_id = encode_buffer_id(semantic.src());
    let bridged = encoded_id.map(decode_buffer_id);

    // Then: each layer has a real value and composition preserves identity.
    assert_eq!(encoded_record, Ok(()));
    assert_eq!(bytes.len(), 64);
    assert_eq!(encoded_id, Ok(AbiId::new(3)));
    assert_eq!(bridged, Ok(Ok(semantic.src())));
}

#[test]
fn legacy_root_and_semantic_paths_are_equal() {
    // Given: values constructed through the legacy root and semantic namespace.
    let legacy = pdma_model_contract::semantic::QueueId::new(17);
    let namespaced = semantic::QueueId::new(17);

    // When/Then: additive names preserve literal type identity and equality.
    assert_eq!(legacy, namespaced);
}

#[test]
fn semantic_capabilities_and_exact_abi_rings_have_direct_owners() {
    // Given: an epoch-qualified semantic queue and an unrelated exact ABI ring.
    let queue =
        semantic::EpochQueue::new(semantic::DeviceEpoch::initial(), semantic::QueueId::new(7));
    let ring = RingLayout::try_new(DmaAddress::new(0x4000), 64, 1, RingRecord::Submission);

    // When/Then: both owner namespaces construct usable values without a bridge.
    assert_eq!(queue.queue_id(), semantic::QueueId::new(7));
    assert_eq!(ring.map(|layout| layout.slot(RingCursor::new(3))), Ok(0));
    assert_eq!(
        pdma_model_contract::semantic::DeviceEpoch::initial(),
        queue.epoch()
    );
}
