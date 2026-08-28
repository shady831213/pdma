use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues,
};

/// Encodes a command into a test-owned descriptor array.
pub fn encode(command: AbiCommand) -> Result<[u8; 64], pdma_model_contract::abi::AbiEncodeError> {
    let mut bytes = [0_u8; 64];
    command.encode(&mut bytes)?;
    Ok(bytes)
}

/// Replaces one checked two-byte fixture field.
pub fn replace_u16(bytes: &mut [u8; 64], offset: usize, value: u16) -> bool {
    let Some(field) = bytes.get_mut(offset..offset + 2) else {
        return false;
    };
    field.copy_from_slice(&value.to_le_bytes());
    true
}

/// Replaces one checked eight-byte fixture field.
pub fn replace_u64(bytes: &mut [u8; 64], offset: usize, value: u64) -> bool {
    let Some(field) = bytes.get_mut(offset..offset + 8) else {
        return false;
    };
    field.copy_from_slice(&value.to_le_bytes());
    true
}

/// Creates all four valid commands represented by the golden vectors.
pub fn commands() -> [AbiCommand; 4] {
    let broadcast = AbiCommand::from_payload(
        0x0f1e_2d3c_4b5a_6978,
        AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
            source_dma_address: 0xfedc_ba98_7654_3210,
            destination_list_dma_address: 0x1020_3040_5060_7008,
            length: 0x5566_7788.into(),
            destination_count: 3.into(),
        }),
    );
    [
        AbiCommand::from_payload(
            0x0123_4567_89ab_cdef,
            AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                source_dma_address: 0x1020_3040_5060_7080,
                destination_dma_address: 0x90a0_b0c0_d0e0_f000,
                length: 0x1122_3344.into(),
            }),
        ),
        AbiCommand::from_payload(
            0x1021_3243_5465_7687,
            AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
                source_dma_address: 0x8877_6655_4433_2211,
                destination_dma_address: 0xf0e0_d0c0_b0a0_9080,
                rows: 0x0102_0304.into(),
                row_bytes: 0x1112_1314.into(),
                source_stride: 0x2122_2324.into(),
                destination_stride: 0x3132_3334.into(),
            }),
        ),
        broadcast,
        AbiCommand::from_payload(
            0x8897_a6b5_c4d3_e2f1,
            AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
                source_a_dma_address: 0x1112_1314_1516_1718,
                source_b_dma_address: 0x2122_2324_2526_2728,
                destination_dma_address: 0x3132_3334_3536_3738,
                length: 0x4546_4748.into(),
            }),
        ),
    ]
}
