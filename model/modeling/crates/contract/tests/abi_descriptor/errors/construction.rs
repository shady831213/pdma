use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadValues,
    DescriptorViolationKind,
};

use super::{assert_decode_violation, support};

pub(super) fn assert_boundary() {
    let zero_count = AbiCommand::from_payload(
        1,
        AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
            source_dma_address: 2,
            destination_list_dma_address: 8,
            length: 0.into(),
            destination_count: 0.into(),
        }),
    );
    let misaligned = AbiCommand::from_payload(
        1,
        AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
            source_dma_address: 2,
            destination_list_dma_address: 11,
            length: 0.into(),
            destination_count: 1.into(),
        }),
    );
    let Ok(zero_count) = support::encode(zero_count) else {
        return;
    };
    let Ok(misaligned) = support::encode(misaligned) else {
        return;
    };
    assert_decode_violation(
        &zero_count,
        DescriptorViolationKind::Scalar(
            AbiCommandPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
                .at_byte(AbiCommand::WORD3_OFFSET),
        ),
    );
    assert_decode_violation(
        &misaligned,
        DescriptorViolationKind::Scalar(
            AbiCommandPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
                .at_byte(AbiCommand::WORD1_OFFSET),
        ),
    );
}
