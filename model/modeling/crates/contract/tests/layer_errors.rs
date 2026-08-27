//! Contract error boundary tests.

use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, AbiDecodeError, DescriptorDecodeError, DescriptorHeaderSchema,
    DescriptorViolationKind,
};
use pdma_model_contract::semantic::SemanticError;
use std::error::Error;

#[test]
fn semantic_errors_are_layer_owned_leaf_errors() {
    // Given: one semantic ring validation error.
    let error = SemanticError::ZeroCapacity;

    // When/Then: it is a typed leaf at the semantic boundary.
    assert!(error.source().is_none());
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
