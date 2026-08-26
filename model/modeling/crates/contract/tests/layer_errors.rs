//! Layer-owned contract error and pure bridge behavior.

use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::abi::{
    ABI_VERSION, AbiAddressError, AbiCommand, AbiDecodeError, AbiEncodeError,
    DescriptorDecodeError, DescriptorHeaderSchema, DescriptorViolationKind,
};
use pdma_model_contract::bridge::{
    BridgeError, CopyBufferContext, CopyRuntimeAddresses, decode_copy_command, encode_copy_command,
};
use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRingId, CopyCommand, CopyRequest, QueueId, SemanticError,
};
use std::error::Error;

#[test]
fn bridge_wraps_identifier_range_with_typed_source() {
    // Given: a semantic ID that cannot fit ABI-v1 on wide targets.
    let error = BridgeError::from(AbiEncodeError::UnrepresentableId {
        value: u128::from(u64::MAX) + 1,
    });

    // When: the standard source chain is inspected.
    let source = error.source();

    // Then: the bridge wrapper exposes the exact ABI cause.
    assert_eq!(
        source.and_then(|cause| cause.downcast_ref()),
        Some(&AbiEncodeError::UnrepresentableId {
            value: u128::from(u64::MAX) + 1,
        })
    );
}

#[test]
fn semantic_errors_are_layer_owned_leaf_errors() {
    // Given: one semantic ring validation error.
    let error = SemanticError::ZeroCapacity;

    // When/Then: it is a typed leaf at the semantic boundary.
    assert!(error.source().is_none());
}

#[test]
fn every_bridge_wrapper_exposes_its_typed_cause() {
    // Given: each cause-carrying bridge wrapper and one bridge-owned leaf.
    let encode = BridgeError::from(AbiEncodeError::InvalidRecordMetadata);
    let decode = BridgeError::from(AbiDecodeError::InvalidRecordMetadata);
    let address = BridgeError::from(AbiAddressError::RangeOverflow {
        base: u64::MAX,
        length: 1,
    });
    let descriptor =
        BridgeError::from(AbiCommand::decode(&[0_u8; 1]).expect_err("short descriptor must fail"));
    let locator = BridgeError::MissingEventLocator;

    // When/Then: each wrapper exposes the concrete cause and the bridge leaf has none.
    assert!(
        encode
            .source()
            .and_then(|cause| cause.downcast_ref::<AbiEncodeError>())
            .is_some()
    );
    assert!(
        decode
            .source()
            .and_then(|cause| cause.downcast_ref::<AbiDecodeError>())
            .is_some()
    );
    assert!(
        address
            .source()
            .and_then(|cause| cause.downcast_ref::<AbiAddressError>())
            .is_some()
    );
    assert!(
        descriptor
            .source()
            .and_then(|cause| cause.downcast_ref::<DescriptorDecodeError>())
            .is_some()
    );
    assert!(locator.source().is_none());
}

#[test]
fn copy_bridge_round_trips_with_explicit_address_and_identity_context() {
    // Given: semantic identities and explicit runtime addresses.
    let command = CopyCommand::new(
        CommandId::new(7),
        QueueId::new(3),
        CompletionRingId::new(5),
        CopyRequest::new(BufferId::new(11), BufferId::new(12), 64),
    );
    let addresses =
        CopyRuntimeAddresses::new(MemoryAddress::new(0x1000), MemoryAddress::new(0x2000));
    let context = CopyBufferContext::from_command(command);

    // When: bridge output is encoded, decoded, and rebuilt semantically.
    let exact = encode_copy_command(command, addresses).expect("copy bridge must encode");
    let mut bytes = [0_u8; 64];
    exact
        .encode(&mut bytes)
        .expect("descriptor buffer is exact");
    let decoded = AbiCommand::decode(&bytes).expect("descriptor bytes must decode");
    let (round_trip, decoded_addresses) =
        decode_copy_command(decoded, context).expect("copy bridge must decode");

    // Then: exact bytes retain the ABI address and semantic values round trip.
    assert_eq!(round_trip, command);
    assert_eq!(decoded_addresses, addresses);
    assert_eq!(&bytes[16..24], &0x1000_u64.to_le_bytes());
}

#[test]
fn descriptor_version_precedes_opcode_for_literal_malformed_bytes() {
    // Given: a readable descriptor with both version and opcode malformed.
    let mut bytes = [0_u8; 64];
    bytes[0..2].copy_from_slice(&(ABI_VERSION + 1).to_le_bytes());
    bytes[2..4].copy_from_slice(&64_u16.to_le_bytes());
    bytes[4..6].copy_from_slice(&u16::MAX.to_le_bytes());

    // When: exact ABI decoding validates it.
    let error = AbiCommand::decode(&bytes).expect_err("malformed descriptor must fail");

    // Then: literal version validation wins before opcode validation.
    assert!(matches!(
        error,
        DescriptorDecodeError::Violation(violation)
            if violation.kind() == DescriptorViolationKind::Header(
                DescriptorHeaderSchema::VERSION_IDENTITY
                    .at_byte(DescriptorHeaderSchema::VERSION_OFFSET)
            )
                && violation.observed() == u64::from(ABI_VERSION + 1)
                && violation.expected() == u64::from(ABI_VERSION)
    ));
}

#[test]
fn descriptor_codec_wrapper_exposes_typed_source() {
    // Given: a short exact descriptor.
    let error = AbiCommand::decode(&[0_u8; 1]).expect_err("short descriptor must fail");

    // When: the wrapper's standard source chain is inspected.
    let source = error.source();

    // Then: the exact codec error is observable as its typed cause.
    assert!(matches!(
        source.and_then(|cause| cause.downcast_ref::<AbiDecodeError>()),
        Some(AbiDecodeError::BufferTooShort { actual: 1, .. })
    ));
}
