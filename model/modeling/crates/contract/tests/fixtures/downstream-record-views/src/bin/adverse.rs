use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, DESCRIPTOR_SIZE, DescriptorDecodeError, DescriptorSchemaViewMut,
};

const COPY: [u8; 64] = [
    1, 0, 1, 0, 0, 0, 64, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 32,
    0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn set_u16(bytes: &mut [u8; 64], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u64(bytes: &mut [u8; 64], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn main() {
    let mut version = COPY;
    set_u16(&mut version, 0, ABI_VERSION + 1);
    set_u16(&mut version, 6, DESCRIPTOR_SIZE - 1);
    set_u16(&mut version, 2, 9);
    let mut opcode = COPY;
    set_u16(&mut opcode, 2, 9);
    set_u16(&mut opcode, 4, 1);
    set_u64(&mut opcode, 40, 1);
    let mut flags = COPY;
    set_u16(&mut flags, 4, 1);
    set_u64(&mut flags, 40, 1);
    let mut reserved = COPY;
    set_u64(&mut reserved, 40, 1);
    set_u64(&mut reserved, 32, u64::from(u32::MAX) + 1);
    let mut scalar = COPY;
    set_u64(&mut scalar, 32, u64::from(u32::MAX) + 1);
    let observed = [
        matches!(AbiCommand::decode(&COPY[..63]), Err(DescriptorDecodeError::Codec(_))),
        matches!(AbiCommand::decode(&version), Err(DescriptorDecodeError::InvalidVersion { .. })),
        matches!(AbiCommand::decode(&opcode), Err(DescriptorDecodeError::InvalidOpcode { .. })),
        matches!(AbiCommand::decode(&flags), Err(DescriptorDecodeError::InvalidFlags { .. })),
        matches!(AbiCommand::decode(&reserved), Err(DescriptorDecodeError::InvalidReserved { .. })),
        matches!(AbiCommand::decode(&scalar), Err(DescriptorDecodeError::ValueOutOfRange { .. })),
    ];
    assert!(observed.into_iter().all(|value| value));
    let mut destination = [0x5a_u8; 63];
    let before = destination;
    let write = DescriptorSchemaViewMut::new(&mut destination).and_then(|mut view| {
        let mut header = view.header_mut()?;
        header.set_version(ABI_VERSION)
    });
    assert!(write.is_err());
    println!("short,version,opcode,flags,reserved,scalar;unchanged={}", destination == before);
}
