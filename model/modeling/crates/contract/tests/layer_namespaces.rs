//! Compile and value proofs for the additive contract namespaces.

use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, DmaAddress, DmaRegion,
    RingLayout,
};
use pdma_model_contract::semantic::{self, BufferId, RingCursor};

#[test]
fn semantic_and_abi_are_independent_and_conversion_is_owner_defined() {
    // Given: one semantic request and one unrelated exact ABI command.
    let semantic = semantic::CopyRequest::new(BufferId::new(3), BufferId::new(5), 8);
    let abi = AbiCommand::from_payload(
        11,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: 0x100,
            destination_dma_address: 0x200,
            length: 8.into(),
        }),
    );

    // When: the exact command is encoded and one ID uses its owning checked conversions.
    let mut bytes = [0_u8; 64];
    let encoded_record = abi.encode(&mut bytes);
    let encoded_id = u64::try_from(semantic.src().raw());
    let decoded_id = encoded_id.map(|id| usize::try_from(id).map(BufferId::new));

    // Then: each layer has a real value and composition preserves identity.
    assert_eq!(encoded_record, Ok(()));
    assert_eq!(bytes.len(), 64);
    assert_eq!(encoded_id, Ok(3));
    assert_eq!(decoded_id, Ok(Ok(semantic.src())));
}

#[test]
fn fully_qualified_and_imported_semantic_paths_are_equal() {
    // Given: values constructed through fully qualified and imported semantic paths.
    let fully_qualified = pdma_model_contract::semantic::QueueId::new(17);
    let imported = semantic::QueueId::new(17);

    // When/Then: one semantic owner preserves literal type identity and equality.
    assert_eq!(fully_qualified, imported);
}

#[test]
fn semantic_capabilities_and_exact_abi_rings_have_direct_owners() {
    // Given: an epoch-qualified semantic queue and an unrelated exact ABI ring.
    let queue =
        semantic::EpochQueue::new(semantic::DeviceEpoch::initial(), semantic::QueueId::new(7));
    let region = DmaRegion::try_new(DmaAddress::new(0x4000), 64).expect("exact region must fit");
    let ring = RingLayout::<AbiCommand>::try_new(region, 1);

    // When/Then: both owner namespaces construct usable values without a translation.
    assert_eq!(queue.queue_id(), semantic::QueueId::new(7));
    assert_eq!(
        ring.map(|layout| layout.slot(RingCursor::new(3).raw())),
        Ok(0)
    );
    assert_eq!(
        pdma_model_contract::semantic::DeviceEpoch::initial(),
        queue.epoch()
    );
}
