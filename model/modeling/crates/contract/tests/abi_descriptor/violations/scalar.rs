use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadStridedCopyValues, AbiCommandPayloadVectorAddValues, AbiEventKind,
    DescriptorViolationKind, MAX_TRANSFER_BYTES,
};

use super::{
    DescriptorExpected, Expected, TOO_WIDE, assert_violation, descriptor_tuple, support, vectors,
};

#[test]
fn scalar_violations_map_by_generated_leaf_identity() {
    // Given: every ordinary fixed-width payload scalar stored above its semantic width.
    let inputs = [
        (
            vectors::COPY,
            32,
            AbiCommandPayloadCopyValues::LENGTH_IDENTITY.at_byte(AbiCommand::WORD2_OFFSET),
        ),
        (
            vectors::STRIDED,
            32,
            AbiCommandPayloadStridedCopyValues::ROWS_IDENTITY.at_byte(AbiCommand::WORD2_OFFSET),
        ),
        (
            vectors::STRIDED,
            40,
            AbiCommandPayloadStridedCopyValues::ROW_BYTES_IDENTITY
                .at_byte(AbiCommand::WORD3_OFFSET),
        ),
        (
            vectors::STRIDED,
            48,
            AbiCommandPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY
                .at_byte(AbiCommand::WORD4_OFFSET),
        ),
        (
            vectors::STRIDED,
            56,
            AbiCommandPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY
                .at_byte(AbiCommand::WORD5_OFFSET),
        ),
        (
            vectors::BROADCAST,
            32,
            AbiCommandPayloadBroadcastCopyValues::LENGTH_IDENTITY.at_byte(AbiCommand::WORD2_OFFSET),
        ),
        (
            vectors::VECTOR_ADD,
            40,
            AbiCommandPayloadVectorAddValues::LENGTH_IDENTITY.at_byte(AbiCommand::WORD3_OFFSET),
        ),
    ];

    // When/Then: decoding maps the generated leaf to the exact scalar fault.
    for (template, offset, location) in inputs {
        let mut bytes = template;
        assert!(support::replace_u64(&mut bytes, offset, TOO_WIDE));
        let offset = u64::try_from(offset);
        assert!(offset.is_ok());
        let Ok(offset) = offset else {
            return;
        };
        assert_violation(
            &bytes,
            Expected {
                kind: DescriptorViolationKind::Scalar(location),
                tuple: descriptor_tuple(DescriptorExpected {
                    kind: AbiEventKind::ValueOutOfRange,
                    observed: TOO_WIDE,
                    expected: u64::from(MAX_TRANSFER_BYTES),
                    offset,
                }),
            },
        );
    }
}
