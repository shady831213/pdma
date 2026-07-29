use pdma_model_contract::abi::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiId, AbiStridedCopy, AbiVectorAdd, BroadcastIovas,
    BroadcastShape, CopyIovas, CopyStrides, DmaAddress, StridedCopyShape, VectorIovas,
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
///
/// # Errors
/// Returns an error if the canonical broadcast fixture violates its typed invariants.
pub fn commands() -> Result<[AbiCommand; 4], pdma_model_contract::abi::DescriptorValueError> {
    let broadcast = AbiBroadcastCopy::try_new(
        AbiId::new(0x0f1e_2d3c_4b5a_6978),
        BroadcastIovas::new(
            DmaAddress::new(0xfedc_ba98_7654_3210),
            DmaAddress::new(0x1020_3040_5060_7008),
        ),
        BroadcastShape::new(0x5566_7788, 3),
    )?;
    Ok([
        AbiCommand::Copy(AbiCopy::new(
            AbiId::new(0x0123_4567_89ab_cdef),
            CopyIovas::new(
                DmaAddress::new(0x1020_3040_5060_7080),
                DmaAddress::new(0x90a0_b0c0_d0e0_f000),
            ),
            0x1122_3344,
        )),
        AbiCommand::StridedCopy(AbiStridedCopy::new(
            AbiId::new(0x1021_3243_5465_7687),
            CopyIovas::new(
                DmaAddress::new(0x8877_6655_4433_2211),
                DmaAddress::new(0xf0e0_d0c0_b0a0_9080),
            ),
            StridedCopyShape::new(
                0x0102_0304,
                0x1112_1314,
                CopyStrides::new(0x2122_2324, 0x3132_3334),
            ),
        )),
        AbiCommand::BroadcastCopy(broadcast),
        AbiCommand::VectorAdd(AbiVectorAdd::new(
            AbiId::new(0x8897_a6b5_c4d3_e2f1),
            VectorIovas::new(
                DmaAddress::new(0x1112_1314_1516_1718),
                DmaAddress::new(0x2122_2324_2526_2728),
                DmaAddress::new(0x3132_3334_3536_3738),
            ),
            0x4546_4748,
        )),
    ])
}
